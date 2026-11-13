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
    /** Activity rows: the engine's transaction kinds. */
    static String kindLabel(String kind){
        switch(kind){
            case "registration":return "Mining registration";
            case "rejoin":return "Mining rejoin · exit";
            case "reactivation":return "Mining reactivation";
            case "topup":return "Bond top-up";
            case "contest":return "Mining registration · contest";
            default:return "Sent PCOL";
        }
    }
    /** The mining card: blocks and bond, and on the v3 network the probation bond still to accrue and the reactivation
     *  clock (V3-09): the last reactivation, and the next allowed height only while it is still ahead (the engine leaves
     *  allowedFrom out once it has passed). Empty strings leave a line or part out. */
    static String minedText(String blocks,String bond,String bondHeld,String remaining,String lastAt,String allowedFrom){
        String text="Accepted blocks this session: "+blocks+"\nRegistration bond: "+bond+" PCOL";
        if(!remaining.isEmpty())text+="\nBond held: "+bondHeld+" PCOL · still to accrue: "+remaining+" PCOL";
        if(!lastAt.isEmpty())text+="\nLast reactivation: block "+lastAt+(allowedFrom.isEmpty()?"":" · next one allowed from block "+allowedFrom);
        return text;
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
    /** A rejoin engine message (type "rejoin": the Scheduled answer to Confirm, and the Reactivating, Retrying, Rejoined and
     *  Stopped events of a running rejoin) as one notice; empty for any other message. */
    static String rejoinNotice(String type,String state,String message){
        if(!type.equals("rejoin"))return "";
        return state.isEmpty()?message:"Mining rejoin · "+state+": "+message;
    }
    /** D-V3-21 option A: the quoted creation burn apart from the gas fee and the amount, and the signed maximum that bounds
     *  both. `burn` empty: the network charges no creation burn (no lines). */
    static java.util.List<String> burnLines(String burn,String gasFee,String bound,String suggested,boolean set,String[] notes){
        java.util.List<String> lines=new java.util.ArrayList<>();if(burn.isEmpty())return lines;
        lines.add("Creation burn (quoted): "+burn+" PCOL");lines.add("Gas fee (at most): "+gasFee+" PCOL");
        lines.add("Maximum fee you sign: "+bound+" PCOL · it bounds the gas and the burn together"+(set?" (your maximum)":" (suggested)"));
        if(set&&!suggested.equals(bound))lines.add("Suggested maximum fee: "+suggested+" PCOL");
        for(String n:notes)lines.add(n);
        return lines;
    }
    static final String MAX_FEE_FIELD="Maximum fee in PCOL (optional: gas and creation burn together; leave empty for the suggested one)";
    static final String CHANGE_MAX_FEE="Change the maximum fee (PCOL)";
    static final String REVIEW_AGAIN="Review with this maximum fee";
    /** V3-10/V3-14: what a contest review states (`kind` seat or pool). */
    static java.util.List<String> contestLines(String kind,String victim,String[] steps,String amount,boolean probation,String target,String note){
        java.util.List<String> lines=new java.util.ArrayList<>();
        lines.add((kind.equals("pool")?"Probation-pool contest of ":"Seat contest of ")+victim);
        for(int i=0;i<steps.length;i++)lines.add((i+1)+". "+steps[i]);
        lines.add("Bond now: "+amount+" PCOL"+(probation?" · filled from your rewards up to "+target+" PCOL":""));
        if(!note.isEmpty())lines.add(note);
        return lines;
    }
    static String candidateLine(String address,String count,String threshold){return address+" · "+count+" shares, threshold "+threshold;}
    static final String SEAT_CANDIDATES="Members you can contest with the full bond";
    static final String POOL_CANDIDATES="Probationers you can contest with a probation bond (every probation seat is taken)";
    static final String CONTEST_BOND="Bond now in PCOL (optional: empty or 0 is a probation bond filled from your rewards)";
    private WalletText(){}
}
