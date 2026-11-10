package org.pocol.node;
/** User-facing texts for peer diagnostics and network upgrades (plan §5). No Android types, so the host JVM test
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
    private WalletText(){}
}
