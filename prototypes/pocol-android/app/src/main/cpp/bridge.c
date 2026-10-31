#include <jni.h>
extern char *pocol_mobile_identity(const char *);
extern char *pocol_mobile_import(const char *, const char *);
extern char *pocol_mobile_inspect_network(const char *);
extern char *pocol_mobile_upgrade(const char *, const char *);
extern char *pocol_mobile_run(const char *);
extern char *pocol_mobile_status(void);
extern char *pocol_mobile_command(const char *);
extern void pocol_mobile_stop(void);
extern void pocol_mobile_free(char *);
static jstring result(JNIEnv *env, char *value) {
    if (!value) return NULL;
    jstring out = (*env)->NewStringUTF(env, value);
    pocol_mobile_free(value);
    return out;
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_identity(JNIEnv *env, jclass cls, jstring home) {
    (void)cls;
    const char *h = (*env)->GetStringUTFChars(env, home, NULL);
    if (!h) return NULL;
    char *out = pocol_mobile_identity(h);
    (*env)->ReleaseStringUTFChars(env, home, h);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_importNetwork(JNIEnv *env, jclass cls, jstring home, jstring path) {
    (void)cls;
    const char *h = (*env)->GetStringUTFChars(env, home, NULL);
    if (!h) return NULL;
    const char *p = (*env)->GetStringUTFChars(env, path, NULL);
    if (!p) { (*env)->ReleaseStringUTFChars(env, home, h); return NULL; }
    char *out = pocol_mobile_import(h, p);
    (*env)->ReleaseStringUTFChars(env, path, p);
    (*env)->ReleaseStringUTFChars(env, home, h);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_run(JNIEnv *env, jclass cls, jstring home) {
    (void)cls;
    const char *h = (*env)->GetStringUTFChars(env, home, NULL);
    if (!h) return NULL;
    char *out = pocol_mobile_run(h);
    (*env)->ReleaseStringUTFChars(env, home, h);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_inspectNetwork(JNIEnv *env, jclass cls, jstring path) {
    (void)cls;
    const char *p = (*env)->GetStringUTFChars(env, path, NULL);
    if (!p) return NULL;
    char *out = pocol_mobile_inspect_network(p);
    (*env)->ReleaseStringUTFChars(env, path, p);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_upgradeNetwork(JNIEnv *env, jclass cls, jstring home, jstring path) {
    (void)cls;
    const char *h = (*env)->GetStringUTFChars(env, home, NULL);
    if (!h) return NULL;
    const char *p = (*env)->GetStringUTFChars(env, path, NULL);
    if (!p) { (*env)->ReleaseStringUTFChars(env, home, h); return NULL; }
    char *out = pocol_mobile_upgrade(h, p);
    (*env)->ReleaseStringUTFChars(env, path, p);
    (*env)->ReleaseStringUTFChars(env, home, h);
    return result(env, out);
}
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_status(JNIEnv *env, jclass cls) { (void)cls; return result(env, pocol_mobile_status()); }
JNIEXPORT void JNICALL Java_org_pocol_node_Native_stop(JNIEnv *env, jclass cls) { (void)env; (void)cls; pocol_mobile_stop(); }
JNIEXPORT jstring JNICALL Java_org_pocol_node_Native_command(JNIEnv *env, jclass cls, jstring command) {
    (void)cls; const char *s=(*env)->GetStringUTFChars(env,command,NULL); if(!s)return NULL;
    char *out=pocol_mobile_command(s);(*env)->ReleaseStringUTFChars(env,command,s);return result(env,out);
}
