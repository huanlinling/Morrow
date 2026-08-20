#!/usr/bin/env python3
"""Headless offline-mode fake client for Morrow e2e (MC 1.20.1, protocol 763).

Usage: fake_client.py chat <name> <message> <hold-seconds>
Join, say one line, hold, then leave. Block break/place coverage comes
from the -Dmorrow.selftest.place self-test mixin, not from this client.

Verified against a real vanilla server. Compression threshold must be
respected (never compress packets below it) and LastSeenMessages is a
fixed 3-byte bitset.
"""
import socket, struct, sys, time, zlib

MODE = sys.argv[1] if len(sys.argv) > 1 else 'chat'
NAME = sys.argv[2] if len(sys.argv) > 2 else 'Steve'
ARG3 = sys.argv[3] if len(sys.argv) > 3 else ''
ARG4 = sys.argv[4] if len(sys.argv) > 4 else '6.0'

HOST, PORT = '127.0.0.1', 25565

def varint(n):
    out = b''
    n &= 0xFFFFFFFF
    while True:
        b = n & 0x7F
        n >>= 7
        if n: out += bytes([b | 0x80])
        else: out += bytes([b]); break
    return out

def parse_varint(buf, off):
    num = 0
    for i in range(5):
        num |= (buf[off+i] & 0x7F) << (7*i)
        if not (buf[off+i] & 0x80): return num, off+i+1
    raise ValueError

def mc_str(s):
    b = s.encode(); return varint(len(b)) + b

threshold = -1
s = socket.create_connection((HOST, PORT), timeout=15)

def read_exact(n):
    d = b''
    while len(d) < n:
        c = s.recv(n-len(d))
        if not c: raise EOFError
        d += c
    return d

def rv():
    num = 0
    for i in range(5):
        b = read_exact(1)[0]
        num |= (b & 0x7F) << (7*i)
        if not (b & 0x80): return num
    raise ValueError

def send(pid, payload=b''):
    body = varint(pid) + payload
    if threshold >= 0 and len(body) >= threshold:
        data = varint(len(body)) + zlib.compress(body)
    elif threshold >= 0:
        data = varint(0) + body
    else:
        data = body
    s.sendall(varint(len(data)) + data)

def rp():
    plen = rv(); raw = read_exact(plen)
    if threshold >= 0:
        dlen, off = parse_varint(raw, 0)
        raw = zlib.decompress(raw[off:]) if dlen else raw[off:]
    pid, off = parse_varint(raw, 0)
    return pid, raw[off:]

# handshake -> login
s.sendall(varint(len(varint(0) + varint(763) + mc_str('localhost') + struct.pack('>H', PORT) + varint(2)))
          + varint(0) + varint(763) + mc_str('localhost') + struct.pack('>H', PORT) + varint(2))
send(0, mc_str(NAME) + b'\x00')  # login start, no UUID
while True:
    pid, payload = rp()
    if pid == 0x03:
        threshold, _ = parse_varint(payload, 0)
    elif pid == 0x02:
        break
    elif pid == 0x00:
        print('[client] login rejected', flush=True); sys.exit(1)

print('[client] logged in', flush=True)
time.sleep(2)

if MODE == 'chat':
    now_ms = int(time.time() * 1000)
    payload = (mc_str(ARG3)
               + struct.pack('>q', now_ms)   # timestamp
               + struct.pack('>q', 0)        # salt
               + b'\x00'                     # no signature
               + varint(0)                   # last-seen offset
               + b'\x00\x00\x00')            # fixed 3-byte bitset
    send(0x05, payload)
    print(f'[client] said: {ARG3}', flush=True)
    time.sleep(float(ARG4))
    s.close()
    print('[client] disconnected', flush=True)

else:
    print('[client] unknown mode', flush=True); sys.exit(2)
