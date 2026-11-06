"""Exercise the real JNI marshaling on a host JVM with a fake Rust C ABI.

No Android device, blockchain, native key or network is involved. This verifies
the string boundary, not Android lifecycle or keystore behavior.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

STUB = r'''
#include <stdlib.h>
#include <string.h>
static char *copy(const char *s) { size_t n=strlen(s)+1; char *p=malloc(n); if(p)memcpy(p,s,n); return p; }
char *pocol_mobile_identity(const char *s) {
    return copy(strcmp(s,"home/\xf0\x9f\x9a\x80")==0 || strcmp(s,"home")==0 ? "{}" : "{\"error\":\"not standard UTF-8\"}");
}
char *pocol_mobile_import(const char *h,const char *p) { (void)h;(void)p;return copy("{}"); }
char *pocol_mobile_inspect_network(const char *p) { (void)p;return copy("{}"); }
char *pocol_mobile_upgrade(const char *h,const char *p) { (void)h;(void)p;return copy("{}"); }
char *pocol_mobile_run(const char *h) { (void)h;return copy("{}"); }
char *pocol_mobile_status(void) { return copy("{\"name\":\"PoCol \xf0\x9f\x9a\x80\"}"); }
char *pocol_mobile_command(const char *s) { return copy(s); }
void pocol_mobile_stop(void) {}
void pocol_mobile_free(char *s) { free(s); }
'''

HARNESS = r'''
package org.pocol.node;
public final class JniBoundaryTest {
    static void equal(String a,String b) { if(!a.equals(b))throw new AssertionError("JNI changed the string: "+a+" != "+b); }
    static void rejected(String input) {
        try { Native.identity(input); }
        catch(IllegalArgumentException expected) { return; }
        throw new AssertionError("JNI accepted a null, NUL-containing or unpaired-surrogate argument");
    }
    public static void main(String[] args) {
        switch(args[0]) {
            case "ascii":
                equal(Native.identity("home"),"{}");equal(Native.importNetwork("home","input"),"{}");
                equal(Native.inspectNetwork("input"),"{}");equal(Native.upgradeNetwork("home","input"),"{}");
                equal(Native.run("home"),"{}");equal(Native.command("{}"),"{}");Native.stop();break;
            case "unicode-output":equal(Native.status(),"{\"name\":\"PoCol \uD83D\uDE80\"}");break;
            case "unicode-input":equal(Native.identity("home/\uD83D\uDE80"),"{}");break;
            case "unicode-boundaries":
                int[] points={1,0x7f,0x80,0x7ff,0x800,0xd7ff,0xe000,0xffff,0x10000,0x10ffff};
                for(int cp:points){String text=new String(Character.toChars(cp));equal(Native.command(text),text);}
                java.util.Random random=new java.util.Random(777900);
                for(int i=0;i<1000;i++){
                    int cp=1+random.nextInt(0x10ffff);
                    if(cp>=0xd800&&cp<=0xdfff)continue;
                    String text="prefix/"+new String(Character.toChars(cp))+"/suffix";
                    equal(Native.command(text),text);
                }
                break;
            case "nul":rejected("home\0other");break;
            case "surrogate":rejected("home\uD800");break;
            case "null":rejected(null);break;
            case "size-bound":
                String limit="a".repeat(1024*1024);
                equal(Native.command(limit),limit);
                rejected(limit+"a");
                rejected("\u0800".repeat(1024*1024/3+1));break;
            default:throw new AssertionError("unknown case");
        }
        System.out.println("PASS "+args[0]);
    }
}
'''


class AndroidJniStrings(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        configured = os.environ.get('POCOL_TEST_JAVA_HOME')
        javac = Path(configured) / 'bin/javac' if configured else shutil.which('javac')
        cc = shutil.which('cc')
        if not javac or not Path(javac).is_file() or not cc:
            raise unittest.SkipTest('Host JDK and C compiler required; set POCOL_TEST_JAVA_HOME')
        cls.jdk = Path(javac).resolve().parents[1]
        if not (cls.jdk / 'include/jni.h').is_file(): raise unittest.SkipTest('JDK JNI headers required')
        cls.temporary = tempfile.TemporaryDirectory(prefix='pocol-jni-test-')
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.work = Path(cls.temporary.name)
        (cls.work / 'stub.c').write_text(STUB)
        (cls.work / 'JniBoundaryTest.java').write_text(HARNESS)
        source = ROOT / 'prototypes/pocol-android/app/src/main'
        subprocess.run([cc, '-shared', '-fPIC', '-Wall', '-Wextra', '-Werror',
            '-I'+str(cls.jdk/'include'), '-I'+str(cls.jdk/'include/linux'),
            str(source/'cpp/bridge.c'), str(cls.work/'stub.c'), '-o', str(cls.work/'libpocol_bridge.so')], check=True)
        subprocess.run([str(javac), '-encoding', 'UTF-8', '-d', str(cls.work),
            str(source/'java/org/pocol/node/Native.java'), str(cls.work/'JniBoundaryTest.java')], check=True)

    def check(self, name):
        p = subprocess.run([str(self.jdk/'bin/java'), '-Xcheck:jni', '-Xmx64m',
            '-XX:ErrorFile='+str(self.work/'hs_err_pid%p.log'),
            '-Djava.library.path='+str(self.work), '-cp', str(self.work),
            'org.pocol.node.JniBoundaryTest', name], cwd=self.work, capture_output=True, text=True,
            encoding='utf-8', errors='replace', timeout=10)
        self.assertEqual(p.returncode, 0, p.stdout+p.stderr)
        self.assertIn('PASS '+name, p.stdout)

    def test_ascii_calls_keep_their_existing_contract(self): self.check('ascii')
    def test_standard_utf8_output_preserves_supplementary_characters(self): self.check('unicode-output')
    def test_java_argument_is_standard_utf8_for_rust(self): self.check('unicode-input')
    def test_utf8_width_boundaries_and_seeded_scalar_roundtrips(self): self.check('unicode-boundaries')
    def test_embedded_nul_cannot_truncate_a_native_argument(self): self.check('nul')
    def test_unpaired_surrogate_is_rejected(self): self.check('surrogate')
    def test_null_argument_raises_java_exception(self): self.check('null')
    def test_native_arguments_are_bounded_by_utf8_bytes(self): self.check('size-bound')


if __name__ == '__main__': unittest.main()
