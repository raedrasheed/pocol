"""Exercise the real JNI marshaling on a host JVM with a fake Rust C ABI.

No Android device, blockchain, native key or network is involved. This verifies
the string boundary, not Android lifecycle or keystore behavior.
"""
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]

STUB = r'''
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static char *copy(const char *s) { size_t n=strlen(s)+1; char *p=malloc(n); if(p)memcpy(p,s,n); return p; }
char *pocol_mobile_identity(const char *s) {
    return copy(strcmp(s,"home/\xf0\x9f\x9a\x80")==0 || strcmp(s,"home")==0 ? "{}" : "{\"error\":\"not standard UTF-8\"}");
}
char *pocol_mobile_import(const char *h,const char *p) { (void)h;(void)p;return copy("{}"); }
char *pocol_mobile_inspect_network(const char *p) { (void)p;return copy("{}"); }
char *pocol_mobile_upgrade(const char *h,const char *p) { (void)h;(void)p;return copy("{}"); }
char *pocol_mobile_inspect_upgrade(const char *h,const char *p) {
    return copy(strcmp(h,"home")==0 && strcmp(p,"input")==0 ? "{\"change\":\"rebind\"}" : "{\"error\":\"arguments swapped\"}");
}
char *pocol_mobile_run(const char *h) { (void)h;return copy("{}"); }
char *pocol_mobile_status(void) { return copy("{\"name\":\"PoCol \xf0\x9f\x9a\x80\"}"); }
char *pocol_mobile_command(const char *s) { return copy(s); }
void pocol_mobile_stop(void) {}
void pocol_mobile_free(char *s) { free(s); }
/* Key backup: echo the received bytes as hex, so the harness checks the exact bytes and lengths. */
static void hex(char *out, const unsigned char *p, size_t n) { static const char d[]="0123456789abcdef"; for(size_t i=0;i<n;i++){out[2*i]=d[p[i]>>4];out[2*i+1]=d[p[i]&15];} out[2*n]=0; }
static char *echo(const char *h, const unsigned char *f, size_t fn, const unsigned char *p, size_t pn, const char *r) {
    if (!f || !p) return copy("{\"error\":\"null pointer\"}");
    size_t size = strlen(h) + strlen(r) + 2*fn + 2*pn + 64; char *out = malloc(size), *a = malloc(2*fn+1), *b = malloc(2*pn+1);
    if (!out || !a || !b) { free(out); free(a); free(b); return NULL; }
    hex(a, f, fn); hex(b, p, pn); snprintf(out, size, "{\"home\":\"%s\",\"file\":\"%s\",\"pass\":\"%s\",\"replace\":\"%s\"}", h, a, b, r);
    free(a); free(b); return out;
}
static const unsigned char none[1] = {0};
char *pocol_mobile_export_key(const char *h, const unsigned char *p, size_t n) { return echo(h, none, 0, p, n, ""); }
char *pocol_mobile_inspect_key_backup(const char *h, const unsigned char *f, size_t fn, const unsigned char *p, size_t pn) { return echo(h, f, fn, p, pn, ""); }
char *pocol_mobile_import_key(const char *h, const unsigned char *f, size_t fn, const unsigned char *p, size_t pn, const char *r) { return echo(h, f, fn, p, pn, r); }
'''

HARNESS = r'''
package org.pocol.node;
public final class JniBoundaryTest {
    static void equal(String a,String b) { if(!a.equals(b))throw new AssertionError("JNI changed the string: "+a+" != "+b); }
    static String hexOf(byte[] b) { StringBuilder s=new StringBuilder(); for(byte x:b)s.append(String.format("%02x",x&0xff)); return s.toString(); }
    static void bytesRejected(Runnable call) {
        try { call.run(); }
        catch(IllegalArgumentException expected) { if(!expected.getMessage().contains("4096 bytes"))throw new AssertionError(expected.getMessage()); return; }
        throw new AssertionError("JNI accepted a null or oversized byte argument");
    }
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
                equal(Native.inspectUpgrade("home","input"),"{\"change\":\"rebind\"}");
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
            case "wallet-text":
                equal(WalletText.peerNote(0,0,""),WalletText.LOOKING);equal(WalletText.peerNote(0,3,null),WalletText.LEGACY_SEEDS);
                equal(WalletText.peerNote(1,3,"peer seed-1: peer runs consensus rules 2"),WalletText.CONSENSUS_REFUSED+"\npeer seed-1: peer runs consensus rules 2");
                equal(WalletText.peerNote(2,0,""),WalletText.CONSENSUS_REFUSED);
                equal(WalletText.activation("2","15000"),"Full mining range at block 15000");equal(WalletText.activation("3","20000"),"Consensus version 3 at block 20000");
                equal(WalletText.outcome("none"),"Already applied");
                equal(WalletText.outcome("rebind"),"Release update: consensus rules and schedule unchanged \u2014 saved; you can connect now");
                if(!WalletText.outcome("activation").contains("coordinated release"))throw new AssertionError("activation text");
                equal(WalletText.restored(true,"0x01"),"Key restored. This wallet now uses account 0x01.");
                equal(WalletText.restored(false,"0x01"),"This backup holds the account this wallet already uses.");
                // V3 wallet texts: activity kinds, the probation bond still to accrue and the reactivation clock, the creation
                // burn review (D-V3-21) and the contest review (V3-10/V3-14).
                equal(WalletText.kindLabel("contest"),"Mining registration \u00b7 contest");equal(WalletText.kindLabel("topup"),"Bond top-up");equal(WalletText.kindLabel("transfer"),"Sent PCOL");
                equal(WalletText.minedText("5","20","","","",""),"Accepted blocks this session: 5\nRegistration bond: 20 PCOL");
                equal(WalletText.minedText("5","20","4","16","120","320"),"Accepted blocks this session: 5\nRegistration bond: 20 PCOL\nBond held: 4 PCOL \u00b7 still to accrue: 16 PCOL\nLast reactivation: block 120 \u00b7 next one allowed from block 320");
                equal(WalletText.minedText("5","20","","","120",""),"Accepted blocks this session: 5\nRegistration bond: 20 PCOL\nLast reactivation: block 120");
                // Review: a rejoin moves no PCOL and keeps the bond locked (not "Bond: 0 PCOL"); rejoin messages and events.
                equal(String.join("|",WalletText.amountRow("rejoin","0")),"Amount|0 PCOL \u00b7 the bond stays locked");equal(String.join("|",WalletText.amountRow("reactivation","0")),"Amount|0 PCOL \u00b7 the bond stays locked");
                equal(String.join("|",WalletText.amountRow("registration","20")),"Bond|20 PCOL");equal(String.join("|",WalletText.amountRow("topup","5")),"Top-up|5 PCOL");equal(String.join("|",WalletText.amountRow("transfer","1")),"Amount|1 PCOL");
                equal(WalletText.rejoinNotice("rejoin","Stopped","The reactivation window passed."),"Mining rejoin \u00b7 Stopped: The reactivation window passed.");
                equal(WalletText.rejoinNotice("rejoin","","x"),"x");equal(WalletText.rejoinNotice("status","Stopped","x"),"");
                if(!WalletText.burnLines("","1","2","2",false,new String[]{"x"}).isEmpty())throw new AssertionError("burn lines without a creation burn");
                java.util.List<String> burn=WalletText.burnLines("0.008","0.0021","0.0101","0.012",true,new String[]{"first","second"});
                equal(String.join("|",burn),"Creation burn (quoted): 0.008 PCOL|Gas fee (at most): 0.0021 PCOL|Maximum fee you sign: 0.0101 PCOL \u00b7 it bounds the gas and the burn together (your maximum)|Suggested maximum fee: 0.012 PCOL|first|second");
                equal(String.join("|",WalletText.burnLines("0","0.0021","0.0022","0.0022",false,new String[0])),"Creation burn (quoted): 0 PCOL|Gas fee (at most): 0.0021 PCOL|Maximum fee you sign: 0.0022 PCOL \u00b7 it bounds the gas and the burn together (suggested)");
                equal(String.join("|",WalletText.contestLines("pool","0x02",new String[]{"a","b"},"0",true,"20","n")),"Probation-pool contest of 0x02|1. a|2. b|Bond now: 0 PCOL \u00b7 filled from your rewards up to 20 PCOL|n");
                equal(String.join("|",WalletText.contestLines("seat","0x03",new String[0],"20",false,"20","")),"Seat contest of 0x03|Bond now: 20 PCOL");
                equal(WalletText.candidateLine("0x03","0","25"),"0x03 \u00b7 0 shares, threshold 25");break;
            case "key-backup":
                // Passphrases cross as standard UTF-8 bytes built from char[] (never a String), files as raw bytes.
                char[] typed="p\u00e4ss \uD83D\uDE80 phrase".toCharArray();
                byte[] pass=KeyBackup.utf8(typed);
                if(!java.util.Arrays.equals(pass,"p\u00e4ss \uD83D\uDE80 phrase".getBytes(java.nio.charset.StandardCharsets.UTF_8)))throw new AssertionError("UTF-8 passphrase");
                String passHex=hexOf(pass);
                equal(Native.exportKey("home",pass),"{\"home\":\"home\",\"file\":\"\",\"pass\":\""+passHex+"\",\"replace\":\"\"}");
                byte[] file=new byte[98];for(int i=0;i<file.length;i++)file[i]=(byte)(i*7);
                equal(Native.inspectKeyBackup("home",file,pass),"{\"home\":\"home\",\"file\":\""+hexOf(file)+"\",\"pass\":\""+passHex+"\",\"replace\":\"\"}");
                equal(Native.importKey("home",file,pass,"0xabc"),"{\"home\":\"home\",\"file\":\""+hexOf(file)+"\",\"pass\":\""+passHex+"\",\"replace\":\"0xabc\"}");
                equal(Native.exportKey("home",new byte[0]),"{\"home\":\"home\",\"file\":\"\",\"pass\":\"\",\"replace\":\"\"}");
                byte[] maxFile=new byte[4096];
                if(!Native.inspectKeyBackup("home",maxFile,pass).contains(hexOf(maxFile)))throw new AssertionError("4096-byte file");
                bytesRejected(()->Native.exportKey("home",null));
                bytesRejected(()->Native.inspectKeyBackup("home",new byte[4097],pass));
                bytesRejected(()->Native.importKey("home",file,null,""));
                // The Java helpers: surrogates, bounds, characters, hex, wiping.
                if(KeyBackup.utf8(new char[]{'a','\uD800'})!=null||KeyBackup.utf8(new char[]{'\uDC00'})!=null)throw new AssertionError("unpaired surrogate");
                char[] longest=new char[1024];java.util.Arrays.fill(longest,'a');if(KeyBackup.utf8(longest).length!=1024)throw new AssertionError("1024 bytes");
                char[] tooLong=new char[1025];java.util.Arrays.fill(tooLong,'a');if(KeyBackup.utf8(tooLong)!=null)throw new AssertionError("over 1024 bytes");
                char[] wide=new char[342];java.util.Arrays.fill(wide,'\u0800');if(KeyBackup.utf8(wide)!=null)throw new AssertionError("over 1024 UTF-8 bytes");
                if(KeyBackup.characters("\uD83D\uDE80abc".toCharArray())!=4)throw new AssertionError("code points");
                if(!java.util.Arrays.equals(KeyBackup.fromHex("0x00ff10"),new byte[]{0,(byte)0xff,0x10})||KeyBackup.fromHex("0x0")!=null||KeyBackup.fromHex("zz")!=null)throw new AssertionError("hex");
                KeyBackup.wipe(typed);KeyBackup.wipe(pass);for(char c:typed)if(c!=0)throw new AssertionError("wipe chars");for(byte b:pass)if(b!=0)throw new AssertionError("wipe bytes");
                if(!KeyBackup.same("abc".toCharArray(),"abc".toCharArray())||KeyBackup.same("abc".toCharArray(),"abd".toCharArray()))throw new AssertionError("same");
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
            str(source/'java/org/pocol/node/Native.java'), str(source/'java/org/pocol/node/WalletText.java'),
            str(source/'java/org/pocol/node/KeyBackup.java'),
            str(cls.work/'JniBoundaryTest.java')], check=True)

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
    def test_peer_diagnostics_and_upgrade_texts_follow_the_status_and_the_dry_run(self): self.check('wallet-text')
    def test_key_backup_bytes_cross_exactly_and_are_bounded(self): self.check('key-backup')


class AndroidBackupRules(unittest.TestCase):
    def test_no_app_data_leaves_through_cloud_backup_or_device_transfer(self):
        # allowBackup="false" stops cloud backup only. On Android 12+ device-to-device transfer reads
        # dataExtractionRules and would otherwise copy the raw key file to a new phone: one key on two devices.
        import xml.etree.ElementTree as ET
        android = '{http://schemas.android.com/apk/res/android}'
        main = ROOT / 'prototypes/pocol-android/app/src/main'
        app = ET.parse(main / 'AndroidManifest.xml').getroot().find('application')
        self.assertEqual(app.get(android + 'allowBackup'), 'false')
        self.assertEqual(app.get(android + 'fullBackupContent'), 'false', 'Android 8 to 11')
        self.assertEqual(app.get(android + 'dataExtractionRules'), '@xml/data_extraction_rules')
        rules = ET.parse(main / 'res/xml/data_extraction_rules.xml').getroot()
        domains = {'root', 'file', 'database', 'sharedpref', 'external',
                   'device_root', 'device_file', 'device_database', 'device_sharedpref'}
        for section in ('cloud-backup', 'device-transfer'):
            part = rules.find(section)
            self.assertIsNotNone(part, section)
            self.assertEqual(part.findall('include'), [], section)
            excluded = {e.get('domain') for e in part.findall('exclude') if e.get('path') == '.'}
            self.assertEqual(excluded, domains, section)


class WalletTexts(unittest.TestCase):
    def test_the_windows_wallet_shows_the_android_texts(self):
        # Plan §5: both wallets tell a lagging node the same thing.
        java = (ROOT / 'prototypes/pocol-android/app/src/main/java/org/pocol/node/WalletText.java').read_text(encoding='utf-8')
        windows = (ROOT / 'prototypes/pocol-windows/Wallet.cs').read_text(encoding='utf-8')
        texts = [s for s in re.findall(r'"((?:[^"\\]|\\.)*)"', java) if len(s) > 12]
        self.assertGreaterEqual(len(texts), 7)
        for text in texts:
            self.assertIn(text, windows)


if __name__ == '__main__': unittest.main()
