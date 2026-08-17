/* M7 7.1: JNI baseline for the Panama comparison.
 * Native side of com.morrow.host.JniBench#add. */
#include <jni.h>

JNIEXPORT jint JNICALL Java_com_morrow_host_JniBench_add
  (JNIEnv *env, jclass cls, jint a, jint b) {
    return a + b;
}
