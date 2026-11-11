package org.pocol.node;
final class Native {
    static { System.loadLibrary("pocol_bridge"); }
    static native String identity(String home);
    static native String importNetwork(String home, String path);
    static native String inspectNetwork(String path);
    static native String upgradeNetwork(String home, String path);
    static native String inspectUpgrade(String home, String path);
    static native String run(String home);
    static native String status();
    static native String command(String json);
    static native void stop();
    /** Key backup (pocold keyfile::backup): passphrases and files cross as byte arrays the caller wipes. Only the
     *  backup's ciphertext comes back (`backup`, hex); the key itself never reaches Java. */
    static native String exportKey(String home, byte[] passphrase);
    static native String inspectKeyBackup(String home, byte[] backup, byte[] passphrase);
    static native String importKey(String home, byte[] backup, byte[] passphrase, String replace);
    private Native() {}
}
