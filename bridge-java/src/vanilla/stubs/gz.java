/** Stub of 1.20.1's obfuscated Registry/DefaultedRegistry — the real gz
 *  is an INTERFACE; declaring it as a class made javac emit
 *  invokevirtual, which blew up at runtime with
 *  IncompatibleClassChangeError (found interface, class expected). */
public interface gz<T> {
    acq b(Object o); // getKey()
}
