#include <jni.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
extern char *pocol_mobile_identity(const char *);
extern char *pocol_mobile_import(const char *, const char *);
extern char *pocol_mobile_inspect_network(const char *);
extern char *pocol_mobile_upgrade(const char *, const char *);
extern char *pocol_mobile_run(const char *);
extern char *pocol_mobile_status(void);
extern char *pocol_mobile_command(const char *);
extern void pocol_mobile_stop(void);
extern void pocol_mobile_free(char *);

#define MAX_ARGUMENT_BYTES (1024u * 1024u)

static void argument_error(JNIEnv *env) {
    jclass type = (*env)->FindClass(env, "java/lang/IllegalArgumentException");
    if (type) {
        (*env)->ThrowNew(env, type, "Native argument must be non-null, bounded Unicode without NUL or unpaired surrogates");
        (*env)->DeleteLocalRef(env, type);
    }
}

/* GetStringUTFChars uses modified UTF-8 (CESU-8 for supplementary code points),
 * whereas Rust accepts standard UTF-8. Convert UTF-16 explicitly and refuse NUL
 * so a Java string can never become a truncated C path/command. */
static char *argument(JNIEnv *env, jstring value) {
    if (!value) { argument_error(env); return NULL; }
    jsize length = (*env)->GetStringLength(env, value);
    if ((size_t)length > MAX_ARGUMENT_BYTES) { argument_error(env); return NULL; }
    const jchar *units = (*env)->GetStringChars(env, value, NULL);
    if (!units) return NULL;
    size_t capacity = (size_t)length * 3;
    if (capacity > MAX_ARGUMENT_BYTES) capacity = MAX_ARGUMENT_BYTES;
    char *out = malloc(capacity + 1);
    if (!out) {
        (*env)->ReleaseStringChars(env, value, units);
        jclass type = (*env)->FindClass(env, "java/lang/OutOfMemoryError");
        if (type) {
            (*env)->ThrowNew(env, type, "Cannot allocate native argument");
            (*env)->DeleteLocalRef(env, type);
        }
        return NULL;
    }
    size_t at = 0;
    for (jsize i = 0; i < length; i++) {
        uint32_t cp = units[i];
        if (cp == 0) goto invalid;
        if (cp >= 0xd800 && cp <= 0xdbff) {
            if (++i >= length || units[i] < 0xdc00 || units[i] > 0xdfff) goto invalid;
            cp = 0x10000 + ((cp - 0xd800) << 10) + (units[i] - 0xdc00);
        } else if (cp >= 0xdc00 && cp <= 0xdfff) goto invalid;
        size_t width = cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
        if (at + width > capacity) goto invalid;
        if (width == 1) out[at++] = (char)cp;
        else if (width == 2) {
            out[at++] = (char)(0xc0 | (cp >> 6));
            out[at++] = (char)(0x80 | (cp & 0x3f));
        } else if (width == 3) {
            out[at++] = (char)(0xe0 | (cp >> 12));
            out[at++] = (char)(0x80 | ((cp >> 6) & 0x3f));
            out[at++] = (char)(0x80 | (cp & 0x3f));
        } else {
            out[at++] = (char)(0xf0 | (cp >> 18));
            out[at++] = (char)(0x80 | ((cp >> 12) & 0x3f));
            out[at++] = (char)(0x80 | ((cp >> 6) & 0x3f));
            out[at++] = (char)(0x80 | (cp & 0x3f));
        }
    }
    out[at] = 0;
    (*env)->ReleaseStringChars(env, value, units);
    return out;
invalid:
    free(out);
    (*env)->ReleaseStringChars(env, value, units);
    argument_error(env);
    return NULL;
}

static jstring result(JNIEnv *env, char *value) {
    if (!value) return NULL;
    jstring out = NULL, encoding = NULL;
    jbyteArray bytes = NULL;
    jclass type = NULL;
    size_t length = strlen(value);
    if (length > INT_MAX) { argument_error(env); goto done; }
    bytes = (*env)->NewByteArray(env, (jsize)length);
    if (!bytes) goto done;
    (*env)->SetByteArrayRegion(env, bytes, 0, (jsize)length, (const jbyte *)value);
    if ((*env)->ExceptionCheck(env)) goto done;
    type = (*env)->FindClass(env, "java/lang/String");
    if (!type) goto done;
    jmethodID constructor = (*env)->GetMethodID(env, type, "<init>", "([BLjava/lang/String;)V");
    if (!constructor) goto done;
    /* Only this fixed ASCII charset name uses modified UTF-8. Rust JSON bytes
     * go through the standard UTF-8 constructor, never through NewStringUTF. */
    encoding = (*env)->NewStringUTF(env, "UTF-8");
    if (!encoding) goto done;
    out = (jstring)(*env)->NewObject(env, type, constructor, bytes, encoding);
done:
    if (encoding) (*env)->DeleteLocalRef(env, encoding);
    if (type) (*env)->DeleteLocalRef(env, type);
    if (bytes) (*env)->DeleteLocalRef(env, bytes);
    pocol_mobile_free(value);
    return out;
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_identity(JNIEnv *env, jclass cls, jstring home) {
    (void)cls;
    char *h = argument(env, home);
    if (!h) return NULL;
    char *out = pocol_mobile_identity(h);
    free(h);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_importNetwork(JNIEnv *env, jclass cls, jstring home, jstring path) {
    (void)cls;
    char *h = argument(env, home);
    if (!h) return NULL;
    char *p = argument(env, path);
    if (!p) { free(h); return NULL; }
    char *out = pocol_mobile_import(h, p);
    free(p);
    free(h);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_run(JNIEnv *env, jclass cls, jstring home) {
    (void)cls;
    char *h = argument(env, home);
    if (!h) return NULL;
    char *out = pocol_mobile_run(h);
    free(h);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_inspectNetwork(JNIEnv *env, jclass cls, jstring path) {
    (void)cls;
    char *p = argument(env, path);
    if (!p) return NULL;
    char *out = pocol_mobile_inspect_network(p);
    free(p);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_upgradeNetwork(JNIEnv *env, jclass cls, jstring home, jstring path) {
    (void)cls;
    char *h = argument(env, home);
    if (!h) return NULL;
    char *p = argument(env, path);
    if (!p) { free(h); return NULL; }
    char *out = pocol_mobile_upgrade(h, p);
    free(p);
    free(h);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_status(JNIEnv *env, jclass cls) { (void)cls; return result(env, pocol_mobile_status()); }
JNIEXPORT void JNICALL Java_org_pocol_node_Native_stop(JNIEnv *env, jclass cls) { (void)env; (void)cls; pocol_mobile_stop(); }
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_command(JNIEnv *env, jclass cls, jstring command) {
    (void)cls; char *s=argument(env,command); if(!s)return NULL;
    char *out=pocol_mobile_command(s);free(s);return result(env,out);
}
