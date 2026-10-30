package org.pocol.node;
import android.app.*;
import android.content.*;
import android.os.*;
import java.io.*;
import java.nio.charset.StandardCharsets;

public final class NodeService extends Service {
    private final Handler main = new Handler(Looper.getMainLooper());
    private Thread worker;
    private PowerManager.WakeLock wake;
    private boolean stopping;
    private final Messenger messages = new Messenger(new Handler(Looper.getMainLooper(), msg -> {
        if (msg.what == 1 && msg.replyTo != null) {
            Message reply = Message.obtain(null, 1);
            Bundle data = new Bundle(); data.putString("status", Native.status()); reply.setData(data);
            try { msg.replyTo.send(reply); } catch (RemoteException ignored) {}
        } else if (msg.what == 2) requestStop();
        else if (msg.what == 3 && msg.replyTo != null) {
            Message reply=Message.obtain(null,3);Bundle data=new Bundle();data.putString("result",Native.command(msg.getData().getString("command","{}")));reply.setData(data);
            try { msg.replyTo.send(reply); } catch (RemoteException ignored) {}
        }
        return true;
    }));
    private final Runnable renew = new Runnable() {
        public void run() {
            if (!stopping && wake != null) { wake.acquire(30 * 60 * 1000L); main.postDelayed(this, 15 * 60 * 1000L); }
        }
    };
    @Override public IBinder onBind(Intent intent) { return messages.getBinder(); }
    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && "STOP".equals(intent.getAction())) { requestStop(); return START_NOT_STICKY; }
        if (worker != null) return START_STICKY;
        NotificationManager nm = getSystemService(NotificationManager.class);
        nm.createNotificationChannel(new NotificationChannel("pocol", "PoCol node", NotificationManager.IMPORTANCE_LOW));
        PendingIntent open = PendingIntent.getActivity(this, 0, new Intent(this, MainActivity.class), PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        PendingIntent stop = PendingIntent.getService(this, 1, new Intent(this, NodeService.class).setAction("STOP"), PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification notification = new Notification.Builder(this, "pocol")
            .setContentTitle("PoCol node running").setContentText("Validating testnet blocks on this phone. Tap to view status.")
            .setSmallIcon(android.R.drawable.stat_notify_sync).setContentIntent(open).setOngoing(true)
            .addAction(new Notification.Action.Builder(null, "Stop", stop).build()).build();
        startForeground(1, notification);
        wake = getSystemService(PowerManager.class).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "PoCol:Node");
        wake.setReferenceCounted(false); renew.run();
        new File(getFilesDir(), "last-result.json").delete();
        worker = new Thread(() -> {
            String result = Native.run(new File(getFilesDir(), "pocol").getAbsolutePath());
            try (FileOutputStream out = openFileOutput("last-result.json", MODE_PRIVATE)) {
                out.write(result.getBytes(StandardCharsets.UTF_8)); out.getFD().sync();
            } catch (IOException ignored) {}
            main.post(() -> { stopForeground(STOP_FOREGROUND_REMOVE); stopSelf(); android.os.Process.killProcess(android.os.Process.myPid()); });
        }, "pocol-host");
        worker.start();
        return START_STICKY;
    }
    private void requestStop() {
        if (stopping) return;
        stopping = true; Native.stop(); main.removeCallbacks(renew);
        if (wake != null && wake.isHeld()) wake.release();
        if (worker == null) { stopSelf(); return; }
        // A bounded stop. The journal's existing crash recovery handles forced termination.
        main.postDelayed(() -> { stopForeground(STOP_FOREGROUND_REMOVE); stopSelf(); android.os.Process.killProcess(android.os.Process.myPid()); }, 10_000);
    }
    @Override public void onTimeout(int startId, int type) { requestStop(); stopSelf(); }
    @Override public void onDestroy() { requestStop(); super.onDestroy(); }
}
