package org.pocol.node;
final class Native {
    static { System.loadLibrary("pocol_bridge"); }
    static native String identity(String home);
    static native String importNetwork(String home, String path);
    static native String inspectNetwork(String path);
    static native String upgradeNetwork(String home, String path);
    static native String run(String home);
    static native String status();
    static native String command(String json);
    static native void stop();
    private Native() {}
}
