package org.pocol.node;
/** User-facing texts for peer diagnostics, network upgrades (plan §5), key backup and the mining card and reviews. No Android types, so the host JVM test
 *  (deploy/standalone/test_android_jni.py) covers them; the Windows wallet shows the same texts. */
final class WalletText {
    static final String CONSENSUS_REFUSED="Peers use a different network configuration: apply the published network upgrade file (Settings → Upgrade) or update PoCol.";
    static final String LOOKING="Looking for peers. Your balance may be out of date.";
    /** While no peer is connected: consensus refusals, with the node's last reason (VA-SPEC §2.5, attack A2-6). */
    static String peerNote(long consensusRefused,String reason){
        if(consensusRefused>0)return reason==null||reason.isEmpty()?CONSENSUS_REFUSED:CONSENSUS_REFUSED+"\n"+reason;
        return LOOKING;
    }
    /** One line per fork entry an upgrade file appends. */
    static String activation(String version,String height){
        return "Consensus version "+version+" at block "+height;
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
    static final String BACKUP_PASSPHRASE="Choose a passphrase of at least 12 characters that is not a repeated pattern, a sequence or a common password. It is never stored: if you forget it, the backup cannot be opened.";
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
    /** Activity rows: the engine's transaction kinds (a registration by replacement is a "registration"). */
    static String kindLabel(String kind){
        switch(kind){
            case "registration":return "Mining registration";
            case "rejoin":return "Mining rejoin · exit";
            case "reactivation":return "Mining reactivation";
            case "topup":return "Bond top-up";
            default:return "Sent PCOL";
        }
    }
    /** The mining card: blocks and bond, and on the v3 network the bond an accruing record still earns from its mining
     *  rewards (no deadline) and the reactivation clock (V3-09): the last reactivation, and the next allowed height only
     *  while it is still ahead (the engine leaves allowedFrom out once it has passed). Empty strings leave a line or part
     *  out. */
    static String minedText(String blocks,String bond,String bondHeld,String remaining,String lastAt,String allowedFrom){
        String text="Accepted blocks this session: "+blocks+"\nRegistration bond: "+bond+" PCOL";
        if(!remaining.isEmpty())text+="\nBond held: "+bondHeld+" PCOL · still to earn from mining: "+remaining+" PCOL";
        if(!lastAt.isEmpty())text+="\nLast reactivation: block "+lastAt+(allowedFrom.isEmpty()?"":" · next one allowed from block "+allowedFrom);
        return text;
    }
    /** The full set (v3 network: every seat taken). `refundAt` non-empty: this account is out of the active set while the set
     *  is full (replaced by a newcomer, or exited), so it cannot reactivate and waits for its bond refund (the engine's rejoin
     *  step "full"). Otherwise, for an identity without a record, whether a registration replaces an inactive member
     *  (seatsFull, removableSeats, registerReplaces). Empty when nothing applies. */
    static String seatText(boolean seatsFull,long removable,boolean replaces,boolean registered,String refundAt){
        if(!refundAt.isEmpty())return "Replaced or exited while every seat is taken: wait for the bond refund at block "+refundAt+", then register again.";
        if(!seatsFull||registered)return "";
        return replaces?"All seats are taken: a registration replaces an inactive member ("+removable+" can be replaced now). That member's bond is refunded and it may return."
            :"All seats are taken and every member mined or joined recently, so none can be replaced yet. Try again later.";
    }
    /** The review's amount row {label, value} for a quote of `kind`. A rejoin or reactivation moves no PCOL and keeps the
     *  bond locked (the Windows wallet's text). */
    static String[] amountRow(String kind,String amount){
        switch(kind){
            case "rejoin":case "reactivation":return new String[]{"Amount",amount+" PCOL · the bond stays locked"};
            case "registration":return new String[]{"Bond",amount+" PCOL"};
            case "topup":return new String[]{"Top-up",amount+" PCOL"};
            default:return new String[]{"Amount",amount+" PCOL"};
        }
    }
    /** The rejoin review's address label and transaction line (the Windows wallet's ReviewRejoin): the Registry receives
     *  the calls, and the quote's `transactions` is the most this wallet signs for it (the exit, if any, and the
     *  reactivation attempts). */
    static final String REGISTRY_LABEL="Registry";
    static String rejoinTransactions(String count){return "Transactions signed by this wallet: up to "+count;}
    /** A rejoin engine message (type "rejoin": the Scheduled answer to Confirm, and the Reactivating, Retrying, Rejoined and
     *  Stopped events of a running rejoin) as one notice; empty for any other message. */
    static String rejoinNotice(String type,String state,String message){
        if(!type.equals("rejoin"))return "";
        return state.isEmpty()?message:"Mining rejoin · "+state+": "+message;
    }
    /** FN-2b C4: what a registration by replacement states (the quote's replacing.victim, steps and note). */
    static java.util.List<String> replaceLines(String victim,String[] steps,String note){
        java.util.List<String> lines=new java.util.ArrayList<>();
        lines.add("Registration by replacement of "+victim);
        for(int i=0;i<steps.length;i++)lines.add((i+1)+". "+steps[i]);
        if(!note.isEmpty())lines.add(note);
        return lines;
    }
    private WalletText(){}
}
