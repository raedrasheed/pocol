package org.pocol.node;

import java.io.File;
import java.io.FileOutputStream;

/** Offline JNI/ART probe. Never calls identity, import, upgrade, run or stop. */
public final class JniDeviceProbe {
    private static int passed;
    private static int failed;

    private static void equal(String actual, String expected) {
        if (!expected.equals(actual)) throw new AssertionError("Unexpected native result: " + actual);
    }

    private static void rejected(String input) {
        try { Native.inspectNetwork(input); }
        catch (IllegalArgumentException expected) { return; }
        throw new AssertionError("JNI accepted an invalid argument");
    }

    private static String repeat(char value, int count) {
        char[] chars = new char[count];
        java.util.Arrays.fill(chars, value);
        return new String(chars);
    }

    private static void check(String name, Runnable body) {
        try { body.run(); passed++; System.out.println("PASS " + name); }
        catch (Throwable error) {
            failed++;
            System.out.println("FAIL " + name + ": " + error.getClass().getSimpleName() + ": " + error.getMessage());
        }
    }

    private static void invalidInvitation(File path) {
        try {
            if (!path.createNewFile()) throw new AssertionError("Probe file already exists");
            try (FileOutputStream stream = new FileOutputStream(path)) {
                stream.write(new byte[] {'{', '}'});
            }
            equal(Native.inspectNetwork(path.getAbsolutePath()), "{\"error\":\"invalid network invitation\"}");
        } catch (java.io.IOException error) { throw new AssertionError(error); }
        finally { if (path.exists() && !path.delete()) throw new AssertionError("Probe file cleanup failed"); }
    }

    public static void main(String[] args) {
        if (args.length != 1) throw new IllegalArgumentException("One isolated probe directory is required");
        final File directory = new File(args[0]);
        if (!directory.isDirectory()) throw new IllegalArgumentException("Probe directory must already exist");
        check("initial-status", () -> equal(Native.status(), "{\"state\":\"starting\"}"));
        check("unicode-path-input", () -> {
            invalidInvitation(new File(directory, "\u0641\u062d\u0635-\uD83D\uDE80.json"));
            int[] scalars = {1, 0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xe000, 0xffff, 0x10000, 0x10ffff};
            for (int scalar : scalars) {
                String name = "scalar-" + new String(Character.toChars(scalar)) + ".json";
                invalidInvitation(new File(directory, name));
            }
        });
        check("unicode-result-output", () -> {
            String path = new File(directory, "missing-\u0641\u062d\u0635-\uD83D\uDE80.json").getAbsolutePath();
            String result = Native.inspectNetwork(path);
            if (!result.startsWith("{\"error\":\"") || !result.contains("open " + path + ":"))
                throw new AssertionError("Rust-to-Java UTF-8 path was changed: " + result);
        });
        check("null-rejected", () -> rejected(null));
        check("embedded-nul-rejected", () -> rejected("bad\u0000argument"));
        check("unpaired-surrogates-rejected", () -> {
            rejected("\uD800"); rejected("\uDC00"); rejected("a\uD800b"); rejected("a\uDC00b");
        });
        check("utf8-byte-limits", () -> {
            String limit = repeat('a', 1024 * 1024);
            equal(Native.command(limit), "{\"error\":\"Command exceeds 4 KiB\"}");
            rejected(limit + "a");
            // Three-byte UTF-8 characters cross the limit before Java's char count.
            rejected(repeat('\u0800', 1024 * 1024 / 3 + 1));
        });
        check("invalid-command-not-queued", () -> equal(Native.command("{}"), "{\"error\":\"Invalid wallet command\"}"));
        check("final-status", () -> equal(Native.status(), "{\"state\":\"starting\"}"));
        System.out.println("POCOL_JNI_RESULT {\"schema\":\"pocol-jni-probe-v1\",\"passed\":" + passed + ",\"failed\":" + failed + "}");
        if (failed != 0) throw new AssertionError("JNI probe failed: " + failed);
    }

    private JniDeviceProbe() {}
}
