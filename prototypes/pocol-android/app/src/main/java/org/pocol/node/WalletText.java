package org.pocol.node;
/** User-facing texts for peer diagnostics, network upgrades (plan §5) and key backup. No Android types, so the host JVM test
 *  (deploy/standalone/test_android_jni.py) covers them; the Windows wallet shows the same texts. */
final class WalletText {
    static final String CONSENSUS_REFUSED="Peers use a different network configuration: apply the published network upgrade file (Settings → Upgrade) or update PoCol.";
    static final String LEGACY_SEEDS="Seeds still run PoCol 0.4.0; wait for the announced switch.";
    static final String LOOKING="Looking for peers. Your balance may be out of date.";
    /** While no peer is connected: consensus refusals first, with the node's last reason, then 0.4.0 handshakes
     *  (VA-SPEC §2.5, attack A2-6). */
    static String peerNote(long consensusRefused,long legacyHellos,String reason){
        if(consensusRefused>0)return reason==null||reason.isEmpty()?CONSENSUS_REFUSED:CONSENSUS_REFUSED+"\n"+reason;
        return legacyHellos>0?LEGACY_SEEDS:LOOKING;
    }
    /** One line per fork entry an upgrade file appends. */
    static String activation(String version,String height){
        return version.equals("2")?"Full mining range at block "+height:"Consensus version "+version+" at block "+height;
    }
    /** The notice for an inspected file that is already applied ("none"), or after a saved rebind or activation. */
    static String outcome(String change){
        switch(change){
            case "none":return "Already applied";
            case "rebind":return "Release update: consensus rules and schedule unchanged — saved; you can connect now";
            default:return "Network upgrade saved. Connect your wallet when the coordinated release is ready.";
        }
    }
    /** Key backup (CRY-02): the same texts in both wallets. The format is pocold's keyfile::backup. */
    static final String BACKUP_INFO="Back up your key to an encrypted file. Anyone with the file and its passphrase controls your PCOL. Without a backup, a lost device, a reset or an uninstall loses the account for good.";
    static final String BACKUP_PASSPHRASE="Choose a passphrase of at least 12 characters. It is never stored: if you forget it, the backup cannot be opened.";
    static final String BACKUP_STORE="Keep the file and the passphrase in different places. Never share either one, and never send them to anyone who asks.";
    static final String BACKUP_SAVED="Encrypted key backup saved. Keep it and its passphrase apart.";
    static final String RESTORE_REPLACE="Restoring replaces the account this wallet uses now. Its key file is kept in the wallet folder, but move its funds first if you have no backup of it.";
    static final String RESTORE_MINING="Stop mining on any other device that uses the restored key: two devices mining with one key can lose the bond.";
    static final String KEY_DISCONNECT="Disconnect the wallet before backing up or restoring its key.";
    static final String PASSPHRASE_MISMATCH="The passphrases differ. Nothing was saved.";
    /** The notice after a restore (`import-key`): the account is unchanged, or the wallet now uses `address`. */
    static String restored(boolean changed,String address){
        return changed?"Key restored. This wallet now uses account "+address+".":"This backup holds the account this wallet already uses.";
    }
    private WalletText(){}
}
