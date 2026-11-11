package org.pocol.node;

/** Byte handling for the encrypted key backup (pocold keyfile::backup, CRY-02). No Android types, so the host JVM
 *  test (deploy/standalone/test_android_jni.py) covers it. Passphrases are handled as char/byte arrays that are
 *  wiped after use, never as a String, and only the backup's ciphertext reaches Java. The wiping is best effort: the
 *  keyboard and the text field's own buffers can keep copies that Java cannot reach (MainActivity overwrites the
 *  field before clearing it). The passphrase policy beyond the length is Rust's (keyfile::backup). */
final class KeyBackup {
    static final int MIN_PASSPHRASE_CHARS=12, MAX_PASSPHRASE_BYTES=1024, MAX_FILE_BYTES=4096;
    static final String MIME="application/octet-stream";

    /** Standard UTF-8 of `chars`, or null for an unpaired surrogate or more than MAX_PASSPHRASE_BYTES. */
    static byte[] utf8(char[] chars){
        byte[] out=new byte[Math.min(chars.length*3,MAX_PASSPHRASE_BYTES+4)];
        int at=0;
        for(int i=0;i<chars.length;i++){
            int cp=chars[i];
            if(Character.isHighSurrogate(chars[i])){
                if(i+1>=chars.length||!Character.isLowSurrogate(chars[i+1])){wipe(out);return null;}
                cp=Character.toCodePoint(chars[i],chars[++i]);
            }else if(Character.isLowSurrogate(chars[i])){wipe(out);return null;}
            int width=cp<0x80?1:cp<0x800?2:cp<0x10000?3:4;
            if(at+width>MAX_PASSPHRASE_BYTES){wipe(out);return null;}
            if(width==1)out[at++]=(byte)cp;
            else if(width==2){out[at++]=(byte)(0xc0|(cp>>6));out[at++]=(byte)(0x80|(cp&0x3f));}
            else if(width==3){out[at++]=(byte)(0xe0|(cp>>12));out[at++]=(byte)(0x80|((cp>>6)&0x3f));out[at++]=(byte)(0x80|(cp&0x3f));}
            else{out[at++]=(byte)(0xf0|(cp>>18));out[at++]=(byte)(0x80|((cp>>12)&0x3f));out[at++]=(byte)(0x80|((cp>>6)&0x3f));out[at++]=(byte)(0x80|(cp&0x3f));}
        }
        byte[] exact=java.util.Arrays.copyOf(out,at);
        wipe(out);
        return exact;
    }
    /** Characters (code points) in `chars`, the unit of the 12-character export policy. */
    static int characters(char[] chars){
        int n=0;
        for(int i=0;i<chars.length;i++){if(Character.isHighSurrogate(chars[i])&&i+1<chars.length&&Character.isLowSurrogate(chars[i+1]))i++;n++;}
        return n;
    }
    /** Whether two passphrase entries are equal, without making Strings of them. */
    static boolean same(char[] a,char[] b){return java.util.Arrays.equals(a,b);}
    static void wipe(char[] value){if(value!=null)java.util.Arrays.fill(value,'\0');}
    static void wipe(byte[] value){if(value!=null)java.util.Arrays.fill(value,(byte)0);}

    /** The backup bytes from the bridge's `backup` hex (ciphertext only), or null if it is not even-length hex. */
    static byte[] fromHex(String hex){
        if(hex==null)return null;
        if(hex.startsWith("0x"))hex=hex.substring(2);
        if(hex.length()%2!=0||hex.length()/2>MAX_FILE_BYTES)return null;
        byte[] out=new byte[hex.length()/2];
        for(int i=0;i<out.length;i++){
            int hi=Character.digit(hex.charAt(2*i),16),lo=Character.digit(hex.charAt(2*i+1),16);
            if(hi<0||lo<0)return null;
            out[i]=(byte)((hi<<4)|lo);
        }
        return out;
    }
    private KeyBackup(){}
}
