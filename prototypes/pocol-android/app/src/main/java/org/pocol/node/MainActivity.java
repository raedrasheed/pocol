package org.pocol.node;

import android.Manifest;
import android.app.*;
import android.content.*;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.os.*;
import android.text.InputType;
import android.view.*;
import android.widget.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import org.json.*;

/** Native wallet. Only public data crosses the service bridge; signing remains in Rust. */
public final class MainActivity extends Activity {
    private static final int IMPORT_NETWORK=1, UPGRADE_NETWORK=3, SAVE_KEY_BACKUP=4, OPEN_KEY_BACKUP=5;
    private static final int BLUE=Color.rgb(26,169,228), INK=Color.rgb(244,248,251), MUTED=Color.rgb(161,177,195), BG=Color.rgb(6,14,24), PANEL=Color.rgb(11,22,35), SOFT=Color.rgb(12,34,51), LINE=Color.rgb(33,49,64);
    private final Handler main=new Handler(Looper.getMainLooper());
    private Messenger service;
    private boolean bound, updating, requestPending, networkActionPending, keyActionPending;
    /** The encrypted backup (ciphertext only) waiting for the user to pick where it is saved. It is kept in the saved
     *  instance state, so a rotation or process death while the file picker is open does not lose it. */
    private byte[] pendingBackup;
    private static final String PENDING_BACKUP="pendingBackup";
    /** The open dialog of a key action, dismissed (which wipes its fields) when the activity is destroyed. */
    private AlertDialog keyDialog;
    /** The activity on screen. A key action's background result goes to it when the activity that started the action
     *  was destroyed meanwhile (a rotation recreates the activity). Main thread only. */
    private static MainActivity live;
    private String home,address="",tab="Wallet";
    private long responseId,eventId=-1;
    private JSONObject snapshot=new JSONObject();
    private LinearLayout content,nav;
    private TextView balance,connectionStatus,miningStatus,headerStatus,networkBadge;
    private Switch miningSwitch;
    /** The engine offers these (rejoinAvailable, topUpAvailable); hidden otherwise. */
    private Button rejoinButton,topUpButton;
    private AlertDialog quoteDialog;
    private final Messenger receiver=new Messenger(new Handler(Looper.getMainLooper(),msg->{
        if(msg.what==1) update(msg.getData().getString("status","{}"));
        if(msg.what==3) try {JSONObject r=new JSONObject(msg.getData().getString("result","{}"));if(r.has("error")){requestPending=false;notice(r.optString("error"));}}catch(JSONException ignored){}
        return true;
    }));
    private final ServiceConnection connection=new ServiceConnection(){
        public void onServiceConnected(ComponentName name,IBinder binder){service=new Messenger(binder);responseId=0;eventId=-1;}
        public void onServiceDisconnected(ComponentName name){service=null;requestPending=false;render();}
    };
    private final Runnable refresh=new Runnable(){public void run(){
        if(service!=null){Message request=Message.obtain(null,1);request.replyTo=receiver;try{service.send(request);}catch(RemoteException e){service=null;}}
        else {File result=new File(getFilesDir(),"last-result.json");if(result.isFile())try(InputStream in=new FileInputStream(result)){JSONObject r=new JSONObject(new String(readLimited(in),StandardCharsets.UTF_8));if(r.has("error"))headerStatus.setText(r.optString("error"));}catch(Exception ignored){}}
        main.postDelayed(this,2000);
    }};
    private int dp(int n){return (int)(n*getResources().getDisplayMetrics().density+.5f);}
    private GradientDrawable shape(int color,int radius){GradientDrawable d=new GradientDrawable();d.setColor(color);d.setCornerRadius(dp(radius));return d;}
    private GradientDrawable surface(int color,int radius){GradientDrawable d=shape(color,radius);d.setStroke(dp(1),LINE);return d;}
    private android.graphics.drawable.RippleDrawable touch(int color,int radius){return new android.graphics.drawable.RippleDrawable(android.content.res.ColorStateList.valueOf(Color.argb(45,26,169,228)),shape(color,radius),null);}
    private TextView text(String value,int size,int color,boolean bold){TextView t=new TextView(this);t.setText(value);t.setTextSize(size);t.setTextColor(color);t.setTypeface(Typeface.create(bold?"sans-serif-medium":"sans-serif",Typeface.NORMAL));t.setPadding(0,dp(4),0,dp(4));t.setLineSpacing(dp(2),1f);return t;}
    private LinearLayout column(){LinearLayout l=new LinearLayout(this);l.setOrientation(LinearLayout.VERTICAL);return l;}
    private LinearLayout card(LinearLayout parent){LinearLayout c=column();c.setPadding(dp(20),dp(18),dp(20),dp(18));c.setBackground(surface(PANEL,20));LinearLayout.LayoutParams p=new LinearLayout.LayoutParams(-1,-2);p.setMargins(0,0,0,dp(16));parent.addView(c,p);return c;}
    private void gap(LinearLayout p,int n){Space s=new Space(this);p.addView(s,new LinearLayout.LayoutParams(1,dp(n)));}
    private void divider(LinearLayout p){View line=new View(this);line.setBackgroundColor(LINE);LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,dp(1));lp.setMargins(0,dp(14),0,dp(14));p.addView(line,lp);}
    private TextView eyebrow(LinearLayout p,String label){TextView t=text(label,11,MUTED,true);t.setLetterSpacing(.08f);p.addView(t);return t;}
    private void heading(String title,String description){content.addView(text(title,28,INK,true));content.addView(text(description,14,MUTED,false));gap(content,20);}
    private Button button(LinearLayout p,String label,boolean primary,Runnable action){Button b=new Button(this);b.setText(label);b.setAllCaps(false);b.setTextSize(14);b.setTextColor(primary?BG:INK);b.setTypeface(Typeface.create("sans-serif-medium",0));b.setBackground(touch(primary?BLUE:SOFT,14));b.setMinHeight(dp(52));b.setPadding(dp(12),dp(10),dp(12),dp(10));LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(-1,-2);lp.setMargins(0,dp(8),0,0);p.addView(b,lp);b.setOnClickListener(v->action.run());return b;}
    private TextView chip(String value,int color){TextView t=text(value,11,color,true);t.setPadding(dp(10),dp(6),dp(10),dp(6));t.setBackground(surface(SOFT,12));return t;}
    private LinearLayout dialogBody(){LinearLayout c=column();c.setPadding(dp(24),dp(12),dp(24),dp(20));return c;}
    private ScrollView scrollBody(LinearLayout body){ScrollView s=new ScrollView(this);s.setFillViewport(false);s.addView(body);return s;}
    private void styleDialog(AlertDialog dialog){
        if(dialog.getWindow()!=null){dialog.getWindow().setBackgroundDrawable(surface(PANEL,20));dialog.getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);}
        for(int which:new int[]{AlertDialog.BUTTON_POSITIVE,AlertDialog.BUTTON_NEGATIVE}){Button b=dialog.getButton(which);if(b!=null){b.setTextColor(which==AlertDialog.BUTTON_POSITIVE?BLUE:MUTED);b.setAllCaps(false);b.setMinHeight(dp(48));}}
    }
    private EditText input(LinearLayout form,String label,String hint,int inputType){
        TextView caption=text(label,13,MUTED,true);form.addView(caption);EditText field=new EditText(this);field.setId(View.generateViewId());caption.setLabelFor(field.getId());field.setHint(hint);field.setTextColor(INK);field.setHintTextColor(MUTED);field.setTextSize(16);field.setSingleLine(true);field.setInputType(inputType);field.setMinHeight(dp(54));field.setPadding(dp(14),dp(12),dp(14),dp(12));field.setBackground(surface(BG,12));form.addView(field,new LinearLayout.LayoutParams(-1,-2));gap(form,12);return field;
    }
    @Override public void onCreate(Bundle state){super.onCreate(state);live=this;home=new File(getFilesDir(),"pocol").getAbsolutePath();
        if(state!=null){pendingBackup=state.getByteArray(PENDING_BACKUP);if(pendingBackup!=null)keyActionPending=true;}getWindow().setStatusBarColor(BG);getWindow().setNavigationBarColor(BG);getWindow().getDecorView().setSystemUiVisibility(0);
        address=getPreferences(MODE_PRIVATE).getString("address","");
        if(address.isEmpty())try{JSONObject r=new JSONObject(Native.identity(home));address=r.optString("address");if(!address.isEmpty())getPreferences(MODE_PRIVATE).edit().putString("address",address).apply();else notice(r.optString("error","Cannot create identity"));}catch(JSONException e){notice("Cannot create identity");}
        render();
    }
    private void render(){
        balance=null;connectionStatus=null;miningSwitch=null;miningStatus=null;rejoinButton=null;topUpButton=null;
        LinearLayout root=column();root.setBackgroundColor(BG);root.setPadding(dp(18),dp(12),dp(18),0);
        root.setOnApplyWindowInsetsListener((v,i)->{v.setPadding(dp(18)+i.getSystemWindowInsetLeft(),dp(12)+i.getSystemWindowInsetTop(),dp(18)+i.getSystemWindowInsetRight(),i.getSystemWindowInsetBottom());return i;});setContentView(root);
        LinearLayout top=new LinearLayout(this);top.setGravity(Gravity.CENTER_VERTICAL);root.addView(top,new LinearLayout.LayoutParams(-1,-2));
        ImageView logo=new ImageView(this);logo.setImageResource(R.drawable.pocol_logo);logo.setContentDescription("PoCol Wallet");logo.setScaleType(ImageView.ScaleType.FIT_START);top.addView(logo,new LinearLayout.LayoutParams(0,dp(40),1));
        networkBadge=chip(snapshot.optString("networkName","PoCol network"),BLUE);top.addView(networkBadge);
        headerStatus=text(service==null?"Wallet offline":"Connecting to peers…",12,MUTED,false);root.addView(headerStatus);gap(root,20);
        ScrollView scroll=new ScrollView(this);scroll.setFillViewport(true);scroll.setClipToPadding(false);scroll.setVerticalScrollBarEnabled(false);root.addView(scroll,new LinearLayout.LayoutParams(-1,0,1));content=column();content.setPadding(0,0,0,dp(8));scroll.addView(content);
        if(tab.equals("Wallet")) wallet();else if(tab.equals("Mining")) mining();else if(tab.equals("Activity")) activity();else settings();
        View line=new View(this);line.setBackgroundColor(LINE);root.addView(line,new LinearLayout.LayoutParams(-1,dp(1)));
        nav=new LinearLayout(this);nav.setPadding(0,dp(8),0,dp(8));root.addView(nav);
        String[] names={"Wallet","Activity","Mining","Settings"};String[] icons={"◈","⇄","◇","⚙"};
        for(int n=0;n<names.length;n++){final String name=names[n];boolean active=name.equals(tab);LinearLayout item=column();item.setGravity(Gravity.CENTER);item.setMinimumHeight(dp(64));item.setBackground(touch(active?SOFT:BG,14));item.setFocusable(true);item.setClickable(true);item.setSelected(active);item.setContentDescription(name+(active?", selected":""));TextView icon=text(icons[n],22,active?BLUE:MUTED,false);icon.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);item.addView(icon);TextView label=text(name,11,active?INK:MUTED,active);label.setPadding(0,0,0,dp(4));label.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);item.addView(label);nav.addView(item,new LinearLayout.LayoutParams(0,-2,1));item.setOnClickListener(v->{tab=name;render();});}
        showSnapshot();
    }
    private void wallet(){
        heading("Your wallet.","Your place in the PoCol network.");
        LinearLayout hero=card(content);GradientDrawable bg=new GradientDrawable(GradientDrawable.Orientation.TL_BR,new int[]{SOFT,Color.rgb(9,49,70)});bg.setCornerRadius(dp(22));bg.setStroke(dp(1),Color.rgb(31,83,109));hero.setBackground(bg);
        eyebrow(hero,"AVAILABLE BALANCE");balance=text("—",42,INK,true);balance.setMaxLines(1);balance.setHorizontallyScrolling(false);balance.setAutoSizeTextTypeUniformWithConfiguration(16,42,1,android.util.TypedValue.COMPLEX_UNIT_SP);balance.setTextIsSelectable(true);hero.addView(balance,new LinearLayout.LayoutParams(-1,dp(66)));hero.addView(text("PCOL",16,BLUE,true));
        divider(hero);TextView account=text(shortAddress()+"  ›",13,INK,false);account.setTypeface(Typeface.MONOSPACE);account.setMinHeight(dp(48));account.setGravity(Gravity.CENTER_VERTICAL);account.setFocusable(true);account.setContentDescription("View your public receiving address");hero.addView(account);account.setOnClickListener(v->receive());
        LinearLayout actions=new LinearLayout(this);content.addView(actions);for(String label:new String[]{"↗  Send PCOL","↙  Receive"}){boolean sendAction=label.contains("Send");LinearLayout c=column();LinearLayout.LayoutParams lp=new LinearLayout.LayoutParams(0,-2,1);lp.setMargins(sendAction?0:dp(6),0,sendAction?dp(6):0,dp(20));actions.addView(c,lp);button(c,label,sendAction,()->{if(sendAction)send();else receive();});}
        LinearLayout asset=card(content);eyebrow(asset,"NATIVE ASSET");LinearLayout row=new LinearLayout(this);row.setGravity(Gravity.CENTER_VERTICAL);asset.addView(row);ImageView icon=new ImageView(this);icon.setImageResource(R.drawable.pocol_icon);icon.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);LinearLayout.LayoutParams ip=new LinearLayout.LayoutParams(dp(44),dp(44));ip.setMargins(0,0,dp(12),0);row.addView(icon,ip);LinearLayout info=column();row.addView(info,new LinearLayout.LayoutParams(0,-2,1));info.addView(text("PoCol",18,INK,true));info.addView(text("PCOL · Native currency",12,MUTED,false));asset.addView(text("Validated locally on your device.",12,MUTED,false));
        LinearLayout status=card(content);status.addView(text("Connected, on your terms",18,INK,true));connectionStatus=text("Connect your local node to see its verified balance.",14,MUTED,false);status.addView(connectionStatus);button(status,service==null?"Connect wallet":"Manage mining",false,()->{if(service==null)startNode();else{tab="Mining";render();}});
    }
    private void mining(){
        heading("Mining","Contribute when it works for you.");LinearLayout c=card(content);eyebrow(c,"THIS DEVICE");c.addView(text("Device mining",20,INK,true));miningSwitch=new Switch(this);miningSwitch.setText("Enable mining");miningSwitch.setTextSize(16);miningSwitch.setTextColor(INK);miningSwitch.setMinHeight(dp(56));miningSwitch.setPadding(0,dp(12),0,dp(12));miningSwitch.setThumbTintList(new android.content.res.ColorStateList(new int[][]{new int[]{android.R.attr.state_checked},new int[]{}},new int[]{BLUE,MUTED}));miningSwitch.setTrackTintList(new android.content.res.ColorStateList(new int[][]{new int[]{android.R.attr.state_checked},new int[]{}},new int[]{Color.rgb(20,98,134),LINE}));c.addView(miningSwitch);miningStatus=text("Connect your wallet first.",14,MUTED,false);c.addView(miningStatus);
        miningSwitch.setOnCheckedChangeListener((b,on)->{if(!updating){updating=true;b.setChecked(snapshot.optBoolean("miningEnabled"));updating=false;command(object("method","mining","enabled",on));}});
        divider(c);button(c,"Review mining registration",false,()->command(object("method","quote_register")));
        rejoinButton=button(c,"Rejoin mining",false,()->command(object("method","quote_rejoin")));
        topUpButton=button(c,"Top up bond",false,()->command(object("method","quote_top_up")));
        for(Button b:new Button[]{rejoinButton,topUpButton})b.setVisibility(View.GONE);
        LinearLayout note=card(content);note.addView(text("You stay in control",18,INK,true));note.addView(text("Turning mining off keeps your wallet connected and validating. Turning it on requires an eligible registered account. Registration locks a bond and pays a network fee. On the v3 network the bond can start at 0 PCOL and is earned from your mining rewards, with no deadline. When every seat is taken, a registration replaces a member that has not mined for about an hour: that member's bond is refunded later, and it may register again.",14,MUTED,false));gap(note,10);note.addView(text("Mining uses battery, mobile data and storage. Android may pause the app. Rewards depend on accepted work and are not guaranteed.",13,MUTED,false));
    }
    private void activity(){
        heading("Activity","Transactions submitted from this device");JSONArray items=snapshot.optJSONArray("activity");if(items==null||items.length()==0){LinearLayout c=card(content);c.addView(text("⇄",34,BLUE,false));c.addView(text("Your activity starts here",20,INK,true));c.addView(text("Your transfers and mining registrations will appear here after you submit them.",14,MUTED,false));return;}
        for(int i=0;i<items.length();i++){JSONObject a=items.optJSONObject(i);if(a==null)continue;LinearLayout c=card(content);c.addView(text(WalletText.kindLabel(a.optString("kind")),16,INK,true));TextView amount=text(a.optString("amount")+" PCOL",23,INK,true);amount.setTextIsSelectable(true);c.addView(amount);c.addView(chip(a.optString("state"),BLUE),new LinearLayout.LayoutParams(-2,-2));if(a.has("confirmations"))c.addView(text(a.optLong("confirmations")+" confirmations",12,MUTED,false));divider(c);eyebrow(c,"TRANSACTION HASH");TextView hash=text(a.optString("hash"),11,MUTED,false);hash.setTypeface(Typeface.MONOSPACE);hash.setTextIsSelectable(true);c.addView(hash);}
    }
    private void settings(){
        heading("Settings","Your network. Your account. Your device.");LinearLayout c=card(content);eyebrow(c,"CONNECTION");c.addView(text(snapshot.optString("networkName","PoCol network"),20,INK,true));c.addView(text("Import the shared network file once. Your private key stays on this device.",14,MUTED,false));
        button(c,"Import network file",true,()->pickNetwork(IMPORT_NETWORK));
        button(c,"Upgrade existing network",false,()->pickNetwork(UPGRADE_NETWORK));
        button(c,"Connect wallet",false,this::startNode);button(c,"Disconnect wallet",false,()->{if(service!=null)try{service.send(Message.obtain(null,2));headerStatus.setText("Disconnecting…");}catch(RemoteException ignored){}});
        LinearLayout identity=card(content);eyebrow(identity,"PUBLIC ADDRESS");identity.addView(text("Your account",20,INK,true));TextView a=text(address,12,INK,false);a.setTypeface(Typeface.MONOSPACE);a.setTextIsSelectable(true);identity.addView(a);button(identity,"Copy public address",false,this::copy);divider(identity);identity.addView(text("Stored on this device",15,INK,true));identity.addView(text("This wallet stores its key on this phone. Uninstalling erases the account and local state unless you keep an encrypted backup.",13,MUTED,false));
        LinearLayout keys=card(content);eyebrow(keys,"KEY BACKUP");keys.addView(text("Encrypted backup",20,INK,true));keys.addView(text(WalletText.BACKUP_INFO,13,MUTED,false));gap(keys,6);keys.addView(text(WalletText.BACKUP_STORE,13,MUTED,false));
        button(keys,"Back up key to encrypted file",true,this::backupKey);button(keys,"Restore key from backup",false,this::pickKeyBackup);
        TextView version=text("PoCol Wallet · "+appVersion(),12,MUTED,false);version.setGravity(Gravity.CENTER);content.addView(version);
    }
    private void send(){
        if(service==null){notice("Connect your wallet before sending.");return;}LinearLayout form=dialogBody();EditText to=input(form,"Recipient address","0x…",InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);to.setTypeface(Typeface.MONOSPACE);to.setImeOptions(android.view.inputmethod.EditorInfo.IME_ACTION_NEXT);EditText amount=input(form,"Amount in PCOL","0.00",InputType.TYPE_CLASS_NUMBER|InputType.TYPE_NUMBER_FLAG_DECIMAL);amount.setImeOptions(android.view.inputmethod.EditorInfo.IME_ACTION_DONE);form.addView(text("You will review the recipient, amount and maximum fee before signing.",13,MUTED,false));
        AlertDialog dialog=new AlertDialog.Builder(this).setTitle("Send PCOL").setView(scrollBody(form)).setNegativeButton("Cancel",null).setPositiveButton("Review",null).create();dialog.setOnShowListener(v->{styleDialog(dialog);dialog.getButton(-1).setOnClickListener(b->{if(requestPending)return;command(object("method","quote_send","to",to.getText().toString().trim(),"amount",amount.getText().toString().trim()));dialog.dismiss();});});dialog.show();
    }
    private void receive(){
        LinearLayout c=dialogBody();c.addView(text("Receive PCOL",24,INK,true));c.addView(text("Use this address on the same PoCol network only.",14,MUTED,false));gap(c,12);TextView a=text(address,15,INK,true);a.setTypeface(Typeface.MONOSPACE);a.setPadding(dp(14),dp(16),dp(14),dp(16));a.setBackground(surface(BG,12));a.setTextIsSelectable(true);c.addView(a);gap(c,12);button(c,"Copy address",true,this::copy);button(c,"Share address",false,()->startActivity(Intent.createChooser(new Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT,address),"Share PoCol address")));AlertDialog d=new AlertDialog.Builder(this).setView(scrollBody(c)).setNegativeButton("Done",null).create();d.show();styleDialog(d);
    }
    private void reviewRow(LinearLayout body,String label,String value,boolean account){
        body.addView(text(label,12,MUTED,true));TextView v=text(value,account?13:18,INK,!account);v.setTextIsSelectable(true);if(account)v.setTypeface(Typeface.MONOSPACE);body.addView(v);gap(body,10);
    }
    private static String[] strings(JSONArray a){if(a==null)return new String[0];String[] out=new String[a.length()];for(int i=0;i<out.length;i++)out[i]=a.optString(i);return out;}
    private void review(JSONObject q){
        requestPending=false;String id=q.optString("quoteId"),kind=q.optString("kind");boolean registration=kind.equals("registration"),top=kind.equals("topup"),rejoin=kind.equals("rejoin")||kind.equals("reactivation");
        LinearLayout body=dialogBody();body.addView(chip("REVIEW BEFORE SIGNING",BLUE),new LinearLayout.LayoutParams(-2,-2));gap(body,14);
        // FN-2b C4: at a full set the registration replaces an inactive member (registerReplacing); the engine names it.
        JSONObject replacing=registration?q.optJSONObject("replacing"):null;
        if(replacing!=null){for(String line:WalletText.replaceLines(replacing.optString("victim"),strings(q.optJSONArray("steps")),q.optString("note")))body.addView(text(line,14,INK,false));divider(body);}
        if(rejoin){String[] steps=strings(q.optJSONArray("steps"));for(int i=0;i<steps.length;i++)body.addView(text((i+1)+". "+steps[i],14,INK,false));divider(body);}
        reviewRow(body,"Network",String.valueOf(q.optLong("chainId")),false);reviewRow(body,"To",q.optString("to"),true);divider(body);
        String[] row=WalletText.amountRow(kind,q.optString("amount"));reviewRow(body,row[0],row[1],false);
        if(top)body.addView(text("Bond held now: "+q.optString("bondHeld")+" of "+q.optString("bondTarget")+" PCOL. If rewards fill part of it first, the extra top-up returns to you.",13,MUTED,false));
        if(top&&!q.optString("note").isEmpty())body.addView(text(q.optString("note"),13,MUTED,false));
        if(registration&&q.optBoolean("probation"))body.addView(text("The rest of the "+q.optString("bondTarget")+" PCOL bond is earned from your mining rewards. You can top it up any time.",13,MUTED,false));
        reviewRow(body,rejoin?"Maximum fees":"Maximum fee",q.optString("maximumFee")+" PCOL",false);reviewRow(body,"Maximum total",q.optString("maximumTotal")+" PCOL",false);body.addView(text("Review expires in 60 seconds.",12,MUTED,false));
        quoteDialog=new AlertDialog.Builder(this).setTitle(registration?(replacing!=null?"Review registration by replacement":"Review registration"):top?"Review bond top-up":rejoin?"Review mining rejoin":"Review transfer").setView(scrollBody(body)).setNegativeButton("Cancel",(d,w)->command(object("method","cancel"))).setPositiveButton("Confirm & sign",(d,w)->command(object("method","confirm","quote_id",id))).setOnCancelListener(d->command(object("method","cancel"))).create();quoteDialog.show();styleDialog(quoteDialog);
    }
    private void update(String raw){try{JSONObject v=new JSONObject(raw);if(v.has("address")){snapshot=v;showSnapshot();showEvent(v);long id=v.optLong("responseId");if(id>responseId){responseId=id;JSONObject r=v.optJSONObject("response");if(r!=null){requestPending=false;String type=r.optString("type");if(type.equals("quote"))review(r);else if(type.equals("error"))notice(r.optString("message"));else if(type.equals("rejoin"))notice(WalletText.rejoinNotice(type,r.optString("state"),r.optString("message")));else if(type.equals("submitted"))new AlertDialog.Builder(this).setTitle("Transaction submitted").setMessage("Waiting for network inclusion.\n\n"+r.optString("hash")).setPositiveButton("Done",null).show();}}}else if(v.has("error")){headerStatus.setText(v.optString("error"));}else headerStatus.setText("Preparing local validation…");}catch(JSONException ignored){}}
    private void showSnapshot(){if(headerStatus==null)return;if(networkBadge!=null)networkBadge.setText(snapshot.optString("networkName","PoCol network"));if(service==null){headerStatus.setText("Wallet offline · balances unavailable");return;}if(!snapshot.has("balance"))return;if(tab.equals("Activity")){content.removeAllViews();activity();}headerStatus.setText(snapshot.optInt("peers")+" peers · Block "+snapshot.optLong("height"));if(balance!=null)balance.setText(snapshot.optString("balance","—"));if(connectionStatus!=null)connectionStatus.setText(snapshot.optInt("peers")>0?"Your device is validating blocks. Mining is "+(snapshot.optBoolean("miningEnabled")?"on.":"off."):peerNote());if(miningSwitch!=null){updating=true;miningSwitch.setChecked(snapshot.optBoolean("miningEnabled"));miningSwitch.setEnabled(snapshot.optBoolean("miningEligible")||snapshot.optBoolean("miningEnabled"));updating=false;}if(miningStatus!=null){JSONObject react=snapshot.optJSONObject("reactivation"),step=snapshot.optJSONObject("rejoin");
            String mined=WalletText.minedText(String.valueOf(snapshot.optLong("blocksMined")),snapshot.optString("bond"),snapshot.optString("bondHeld"),snapshot.isNull("bondRemaining")?"":snapshot.optString("bondRemaining"),react==null||react.isNull("lastAt")?"":react.optString("lastAt"),react==null||react.isNull("allowedFrom")?"":react.optString("allowedFrom"));
            String seats=WalletText.seatText(snapshot.optBoolean("seatsFull"),snapshot.optLong("removableSeats"),snapshot.optBoolean("registerReplaces"),snapshot.optBoolean("registered"),step!=null&&step.optString("step").equals("full")?step.optString("refundAt"):"");
            miningStatus.setText(snapshot.optString("miningState")+"\n"+mined+(seats.isEmpty()?"":"\n"+seats));}
        if(rejoinButton!=null)rejoinButton.setVisibility(snapshot.optBoolean("rejoinAvailable")?View.VISIBLE:View.GONE);if(topUpButton!=null)topUpButton.setVisibility(snapshot.optBoolean("topUpAvailable")?View.VISIBLE:View.GONE);}
    /** A running rejoin's events (the engine's 2 s advance: Reactivating, Retrying, Rejoined, Stopped), published apart
     *  from the command answers. The first snapshot after connecting only records the current event. */
    private void showEvent(JSONObject v){long id=v.optLong("eventId");if(eventId<0){eventId=id;return;}if(id<=eventId)return;eventId=id;JSONObject e=v.optJSONObject("event");if(e==null)return;String text=WalletText.rejoinNotice(e.optString("type"),e.optString("state"),e.optString("message"));if(!text.isEmpty())notice(text);}
    /** Why no peer is connected (VA-SPEC §2.5): the wallet status carries the node's refusal counts and last reason. */
    private String peerNote(){JSONObject refused=snapshot.optJSONObject("incompatiblePeers");return WalletText.peerNote(refused==null?0:refused.optLong("consensus"),snapshot.isNull("lastIncompatibleReason")?"":snapshot.optString("lastIncompatibleReason"));}
    private void startNode(){if(!new File(home,"network.pocol.json").isFile()){notice("Import the shared network file in Settings first.");tab="Settings";render();return;}if(Build.VERSION.SDK_INT>=33&&checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED)requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS},2);try{startForegroundService(new Intent(this,NodeService.class));attach();headerStatus.setText("Starting local validation…");}catch(RuntimeException e){notice("Cannot start: "+e.getMessage());}}
    private void command(JSONObject command){if(service==null){notice("Connect your wallet first.");return;}if(requestPending){notice("Please wait for the previous request.");return;}Message m=Message.obtain(null,3);Bundle b=new Bundle();b.putString("command",command.toString());m.setData(b);m.replyTo=receiver;try{service.send(m);requestPending=true;main.postDelayed(()->requestPending=false,10000);}catch(RemoteException e){notice("Wallet service unavailable");}}
    private static JSONObject object(Object... pairs){JSONObject j=new JSONObject();try{for(int i=0;i<pairs.length;i+=2)j.put((String)pairs[i],pairs[i+1]);}catch(JSONException ignored){}return j;}
    private String shortAddress(){return address.length()==42?address.substring(0,8)+"…"+address.substring(36):"Account unavailable";}
    private String appVersion(){try{return getPackageManager().getPackageInfo(getPackageName(),0).versionName;}catch(PackageManager.NameNotFoundException e){return "Version unavailable";}}
    private void copy(){getSystemService(android.content.ClipboardManager.class).setPrimaryClip(ClipData.newPlainText("PoCol address",address));notice("Address copied");}
    private void notice(String message){Toast.makeText(this,message,Toast.LENGTH_LONG).show();}
    private void attach(){if(!bound)bound=bindService(new Intent(this,NodeService.class),connection,0);}
    @Override protected void onStart(){super.onStart();attach();main.post(refresh);}
    @Override protected void onStop(){main.removeCallbacks(refresh);if(bound){unbindService(connection);bound=false;service=null;}super.onStop();}
    @Override protected void onSaveInstanceState(Bundle out){super.onSaveInstanceState(out);if(pendingBackup!=null)out.putByteArray(PENDING_BACKUP,pendingBackup);}
    @Override protected void onDestroy(){
        // A dialog left open would leak its window and never run its dismiss listener, which wipes the passphrase.
        if(keyDialog!=null){AlertDialog d=keyDialog;keyDialog=null;d.dismiss();}
        if(live==this)live=null;
        super.onDestroy();
    }
    private static byte[] readLimited(InputStream in)throws IOException{ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] b=new byte[8192];int n;while((n=in.read(b))!=-1){if(out.size()+n>1048576)throw new IOException("File exceeds 1 MiB");out.write(b,0,n);}return out.toByteArray();}
    private void pickNetwork(int request){
        if(networkActionPending){notice("Finish the current network action first.");return;}
        if(service!=null){notice("Disconnect the wallet, reopen the app, then select the network file.");return;}
        if(request==UPGRADE_NETWORK&&!new File(home,"network.pocol.json").isFile()){notice("Import a network before upgrading it.");return;}
        try{
            networkActionPending=true;
            startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT).setType("application/json").addCategory(Intent.CATEGORY_OPENABLE),request);
        }catch(RuntimeException e){networkActionPending=false;notice("Cannot open file picker: "+e.getMessage());}
    }
    private void reviewNetworkUpgrade(File incoming,JSONObject preview){
        if(isFinishing()||isDestroyed()){incoming.delete();networkActionPending=false;return;}
        // The dry run (`inspect-upgrade`) said what applying the file changes: a rebind or appended fork entries.
        final String change=preview.optString("change");
        LinearLayout body=dialogBody();body.addView(chip(change.equals("rebind")?"REVIEW RELEASE UPDATE":"REVIEW NETWORK UPGRADE",BLUE),new LinearLayout.LayoutParams(-2,-2));gap(body,12);
        reviewRow(body,"Network",preview.optString("networkName","PoCol network"),false);
        reviewRow(body,"Chain ID",preview.optString("chainId"),false);
        JSONArray appended=preview.optJSONArray("appended");
        if(appended!=null)for(int i=0;i<appended.length();i++){JSONArray entry=appended.optJSONArray(i);if(entry!=null&&entry.length()==2)body.addView(text(WalletText.activation(entry.opt(0).toString(),entry.opt(1).toString()),16,INK,true));}
        reviewRow(body,"Genesis hash",preview.optString("genesisHash"),true);
        body.addView(text(change.equals("rebind")?"This release runs the same consensus rules and fork schedule. Saving the new file changes no rule. Your account, private key and stored chain stay on this device.":"Use the shared upgrade file supplied for this PoCol network. The new rules start at the blocks shown. Your account, private key and stored chain stay on this device.",14,MUTED,false));
        body.addView(text("The wallet must be disconnected. The native node checks the file against your existing network before applying it.",13,MUTED,false));
        AlertDialog dialog=new AlertDialog.Builder(this).setTitle("Upgrade existing network?").setView(scrollBody(body))
            .setNegativeButton("Cancel",(d,w)->{incoming.delete();networkActionPending=false;})
            .setOnCancelListener(d->{incoming.delete();networkActionPending=false;})
            .setPositiveButton("Confirm upgrade",(d,w)->{
                if(service!=null){incoming.delete();networkActionPending=false;notice("Disconnect the wallet before upgrading.");return;}
                new Thread(()->{
                    try{
                        JSONObject result=new JSONObject(Native.upgradeNetwork(home,incoming.getAbsolutePath()));
                        main.post(()->{networkActionPending=false;if(result.has("error"))notice(result.optString("error"));else{snapshot=new JSONObject();notice(WalletText.outcome(change));tab="Settings";render();}});
                    }catch(Exception e){main.post(()->{networkActionPending=false;notice("Upgrade failed: "+e.getMessage());});}
                    finally{incoming.delete();}
                },"network-upgrade").start();
            }).create();
        dialog.show();styleDialog(dialog);
    }
    // ---- Encrypted key backup (pocold keyfile::backup, CRY-02). The key never reaches Java: Rust returns the
    // ciphertext for export and takes the file bytes for restore. Passphrases are copied out of the fields into
    // char/byte arrays that are wiped after use, and the fields are overwritten before they are cleared. That is best
    // effort: the keyboard and the field's earlier buffers can keep copies Java cannot reach. Every dialog that shows
    // a passphrase field is FLAG_SECURE (no screenshots, recordings or recents thumbnail).
    private EditText secretInput(LinearLayout form,String label){
        EditText field=input(form,label,"",InputType.TYPE_CLASS_TEXT|InputType.TYPE_TEXT_VARIATION_PASSWORD|InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        // No autofill, no keyboard learning, no long-press menu; a password field already refuses copy and cut.
        field.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO);field.setImeOptions(android.view.inputmethod.EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING);field.setLongClickable(false);
        return field;
    }
    /** The field's characters in a new array; the field is then wiped ({@link #wipe(EditText)}). */
    private static char[] take(EditText field){android.text.Editable e=field.getText();char[] chars=new char[e.length()];e.getChars(0,chars.length,chars,0);wipe(field);return chars;}
    /** Overwrites the field's text in place with filler of the same length, then clears it: clear() alone only resets
     *  the length and leaves the characters in the editor's buffer. Best effort, as above. */
    private static void wipe(EditText field){android.text.Editable e=field.getText();int n=e.length();if(n>0){char[] fill=new char[n];java.util.Arrays.fill(fill,' ');e.replace(0,n,java.nio.CharBuffer.wrap(fill));}e.clear();}
    /** A FLAG_SECURE dialog, tracked as the open key dialog until it is dismissed. */
    private AlertDialog secureDialog(AlertDialog.Builder builder){AlertDialog d=builder.create();if(d.getWindow()!=null)d.getWindow().setFlags(WindowManager.LayoutParams.FLAG_SECURE,WindowManager.LayoutParams.FLAG_SECURE);keyDialog=d;return d;}
    private void keyDialogClosed(DialogInterface d){if(keyDialog==d)keyDialog=null;}
    /** The activity to show a key action's background result: this one, the one that replaced it, or null. */
    private MainActivity shown(){
        if(!isFinishing()&&!isDestroyed())return this;
        MainActivity a=live;
        return a!=null&&a!=this&&!a.isFinishing()&&!a.isDestroyed()?a:null;
    }
    /** Deletes a document the file picker created, when no complete backup could be written to it. */
    private boolean deleteDocument(android.net.Uri uri){try{return android.provider.DocumentsContract.deleteDocument(getContentResolver(),uri);}catch(Exception e){return false;}}
    private boolean keyActionAllowed(){
        if(service!=null){notice(WalletText.KEY_DISCONNECT);return false;}
        if(keyActionPending||networkActionPending){notice("Finish the current action first.");return false;}
        return true;
    }
    private void backupKey(){
        if(!keyActionAllowed())return;
        LinearLayout form=dialogBody();form.addView(text(WalletText.BACKUP_INFO,13,MUTED,false));form.addView(text(WalletText.BACKUP_PASSPHRASE,13,MUTED,false));form.addView(text(WalletText.BACKUP_STORE,13,MUTED,false));gap(form,8);
        EditText first=secretInput(form,"Backup passphrase"),second=secretInput(form,"Repeat the passphrase");
        AlertDialog dialog=secureDialog(new AlertDialog.Builder(this).setTitle("Back up your key").setView(scrollBody(form)).setNegativeButton("Cancel",null).setPositiveButton("Encrypt backup",null));
        dialog.setOnDismissListener(d->{wipe(first);wipe(second);keyDialogClosed(d);});
        dialog.setOnShowListener(v->{styleDialog(dialog);dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(b->{
            char[] a=take(first),c=take(second);
            try{
                if(!KeyBackup.same(a,c)){notice(WalletText.PASSPHRASE_MISMATCH);return;}
                if(KeyBackup.characters(a)<KeyBackup.MIN_PASSPHRASE_CHARS){notice("Use a passphrase of at least 12 characters");return;}
                byte[] pass=KeyBackup.utf8(a);
                if(pass==null){notice("This passphrase cannot be used; choose another one.");return;}
                dialog.dismiss();keyActionPending=true;notice("Encrypting your key backup…");
                new Thread(()->{
                    String raw;
                    try{raw=Native.exportKey(home,pass);}catch(RuntimeException e){raw=null;}finally{KeyBackup.wipe(pass);}
                    final String reply=raw;
                    main.post(()->exported(reply));
                },"key-backup").start();
            }finally{KeyBackup.wipe(a);KeyBackup.wipe(c);}
        });});
        dialog.show();
    }
    private void exported(String raw){
        MainActivity a=shown();
        if(a!=this){keyActionPending=false;if(a!=null&&!a.keyActionPending){a.keyActionPending=true;a.exported(raw);}return;}
        try{
            JSONObject r=new JSONObject(raw==null?"{\"error\":\"Key backup failed\"}":raw);
            if(r.has("error")){keyActionPending=false;notice(r.optString("error"));return;}
            pendingBackup=KeyBackup.fromHex(r.optString("backup"));
            if(pendingBackup==null){keyActionPending=false;notice("Key backup failed");return;}
            startActivityForResult(new Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType(KeyBackup.MIME).putExtra(Intent.EXTRA_TITLE,r.optString("fileName","pocol-key.pocol-backup")),SAVE_KEY_BACKUP);
        }catch(JSONException|RuntimeException e){pendingBackup=null;keyActionPending=false;notice("Key backup failed: "+e.getMessage());}
    }
    private void pickKeyBackup(){
        if(!keyActionAllowed())return;
        try{keyActionPending=true;startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*"),OPEN_KEY_BACKUP);}
        catch(RuntimeException e){keyActionPending=false;notice("Cannot open file picker: "+e.getMessage());}
    }
    private void keyFileResult(int request,int result,Intent data){
        android.net.Uri uri=result==RESULT_OK&&data!=null?data.getData():null;
        if(request==SAVE_KEY_BACKUP){
            final byte[] backup=pendingBackup;pendingBackup=null;
            if(uri==null){keyActionPending=false;notice("Backup not saved.");return;}
            // The picker has already created the file. Unless it ends up holding the complete backup, read back and
            // compared byte for byte, it is deleted: an empty or partial file must never look like a backup.
            new Thread(()->{
                String message=WalletText.BACKUP_SAVED;
                try{
                    if(backup==null)throw new IOException("the app was restarted while the file picker was open. Make the backup again");
                    try(OutputStream out=getContentResolver().openOutputStream(uri,"wt")){if(out==null)throw new IOException("Cannot write file");out.write(backup);out.flush();}
                    byte[] saved;
                    try(InputStream in=getContentResolver().openInputStream(uri)){if(in==null)throw new IOException("Cannot read the saved file back");saved=readLimited(in);}
                    if(!java.util.Arrays.equals(saved,backup))throw new IOException("the saved file does not match the backup");
                }catch(Exception e){message="Backup not saved: "+e.getMessage()+(deleteDocument(uri)?"":". Delete the incomplete file you chose.");}
                final String m=message;main.post(()->{keyActionPending=false;MainActivity a=shown();if(a!=null)a.notice(m);});
            },"key-backup-save").start();
            return;
        }
        if(uri==null){keyActionPending=false;return;}
        keyActionPending=true;
        new Thread(()->{
            try(InputStream in=getContentResolver().openInputStream(uri)){
                if(in==null)throw new IOException("Cannot read file");
                byte[] file=readLimited(in);
                if(file.length>KeyBackup.MAX_FILE_BYTES)throw new IOException("This is not a PoCol key backup");
                main.post(()->{MainActivity a=shown();if(a==this)askRestorePassphrase(file);else{keyActionPending=false;if(a!=null&&!a.keyActionPending){a.keyActionPending=true;a.askRestorePassphrase(file);}}});
            }catch(Exception e){main.post(()->{keyActionPending=false;notice("Backup file failed: "+e.getMessage());});}
        },"key-backup-open").start();
    }
    private void askRestorePassphrase(byte[] file){
        if(isFinishing()||isDestroyed()){keyActionPending=false;return;}
        LinearLayout form=dialogBody();form.addView(text("Enter the passphrase chosen when this backup was made.",13,MUTED,false));gap(form,8);
        EditText field=secretInput(form,"Backup passphrase");
        final boolean[] opened={false};
        AlertDialog dialog=secureDialog(new AlertDialog.Builder(this).setTitle("Restore key from backup").setView(scrollBody(form)).setNegativeButton("Cancel",null).setPositiveButton("Open backup",null));
        dialog.setOnDismissListener(d->{wipe(field);keyDialogClosed(d);if(!opened[0])keyActionPending=false;});
        dialog.setOnShowListener(v->{styleDialog(dialog);dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(b->{
            char[] chars=take(field);byte[] pass=KeyBackup.utf8(chars);KeyBackup.wipe(chars);
            if(pass==null||pass.length==0){notice("Enter the backup passphrase");return;}
            opened[0]=true;dialog.dismiss();notice("Opening backup…");
            new Thread(()->{
                String raw;try{raw=Native.inspectKeyBackup(home,file,pass);}catch(RuntimeException e){raw=null;}
                final String reply=raw;
                main.post(()->inspected(file,pass,reply));
            },"key-backup-inspect").start();
        });});
        dialog.show();
    }
    private void inspected(byte[] file,byte[] pass,String reply){
        MainActivity a=shown();
        if(a!=this){keyActionPending=false;if(a!=null&&!a.keyActionPending){a.keyActionPending=true;a.inspected(file,pass,reply);}else KeyBackup.wipe(pass);return;}
        try{
            JSONObject preview=new JSONObject(reply==null?"{\"error\":\"Key backup failed\"}":reply);
            if(preview.has("error")){KeyBackup.wipe(pass);keyActionPending=false;notice(preview.optString("error"));return;}
            if(preview.optBoolean("replaces"))confirmRestore(file,pass,preview);else restoreKey(file,pass,"");
        }catch(JSONException e){KeyBackup.wipe(pass);keyActionPending=false;notice("Key backup failed");}
    }
    private void confirmRestore(byte[] file,byte[] pass,JSONObject preview){
        if(isFinishing()||isDestroyed()){KeyBackup.wipe(pass);keyActionPending=false;return;}
        String current=preview.optString("current");
        LinearLayout body=dialogBody();body.addView(chip("REVIEW BEFORE REPLACING",BLUE),new LinearLayout.LayoutParams(-2,-2));gap(body,12);
        reviewRow(body,"Current account",current,true);reviewRow(body,"Restored account",preview.optString("address"),true);
        body.addView(text(WalletText.RESTORE_REPLACE,13,MUTED,false));body.addView(text(WalletText.RESTORE_MINING,13,MUTED,false));
        final boolean[] confirmed={false};
        AlertDialog dialog=secureDialog(new AlertDialog.Builder(this).setTitle("Replace this wallet's account?").setView(scrollBody(body))
            .setNegativeButton("Cancel",null).setPositiveButton("Replace account",(d,w)->{confirmed[0]=true;restoreKey(file,pass,current);}));
        dialog.setOnDismissListener(d->{keyDialogClosed(d);if(!confirmed[0]){KeyBackup.wipe(pass);keyActionPending=false;}});
        dialog.show();styleDialog(dialog);
    }
    private void restoreKey(byte[] file,byte[] pass,String replace){
        if(service!=null){KeyBackup.wipe(pass);keyActionPending=false;notice(WalletText.KEY_DISCONNECT);return;}
        new Thread(()->{
            String raw;try{raw=Native.importKey(home,file,pass,replace);}catch(RuntimeException e){raw=null;}finally{KeyBackup.wipe(pass);}
            final String reply=raw;
            main.post(()->{
                keyActionPending=false;
                // The restore has happened whether or not this activity is still on screen: record the address,
                // then show it on the activity that is (after a rotation, the new one).
                MainActivity a=shown();
                try{
                    JSONObject r=new JSONObject(reply==null?"{\"error\":\"Key restore failed\"}":reply);
                    if(r.has("error")){if(a!=null)a.notice(r.optString("error"));return;}
                    String restored=r.optString("address");getPreferences(MODE_PRIVATE).edit().putString("address",restored).apply();
                    address=restored;
                    if(a==null)return;
                    a.address=restored;a.snapshot=new JSONObject();a.tab="Settings";a.render();
                    a.notice(WalletText.restored(r.optBoolean("changed"),restored));
                }catch(JSONException e){if(a!=null)a.notice("Key restore failed");}
            });
        },"key-backup-restore").start();
    }
    @Override protected void onActivityResult(int request,int result,Intent data){
        super.onActivityResult(request,result,data);
        if(request==SAVE_KEY_BACKUP||request==OPEN_KEY_BACKUP){keyFileResult(request,result,data);return;}
        if(request!=IMPORT_NETWORK&&request!=UPGRADE_NETWORK)return;
        if(result!=RESULT_OK||data==null||data.getData()==null){networkActionPending=false;return;}
        new Thread(()->{
            File incoming=null;boolean reviewing=false;
            try(InputStream in=getContentResolver().openInputStream(data.getData())){
                if(in==null)throw new IOException("Cannot read file");
                incoming=File.createTempFile("pocol-network-",".json",getCacheDir());
                try(FileOutputStream out=new FileOutputStream(incoming)){out.write(readLimited(in));out.getFD().sync();}
                if(request==UPGRADE_NETWORK){
                    // A read-only dry run against this home: none, a rebind, or the fork entries it appends.
                    JSONObject preview=new JSONObject(Native.inspectUpgrade(home,incoming.getAbsolutePath()));
                    if(preview.has("error"))throw new IOException(preview.optString("error"));
                    if(preview.optString("change").equals("none"))main.post(()->{networkActionPending=false;notice(WalletText.outcome("none"));});
                    else{final File selected=incoming;reviewing=true;main.post(()->reviewNetworkUpgrade(selected,preview));}
                }else{
                    JSONObject imported=new JSONObject(Native.importNetwork(home,incoming.getAbsolutePath()));
                    main.post(()->{networkActionPending=false;if(imported.has("error"))notice(imported.optString("error"));else{notice("Network imported");startNode();}});
                }
            }catch(Exception e){main.post(()->{networkActionPending=false;notice("Network file failed: "+e.getMessage());});}
            finally{if(!reviewing&&incoming!=null)incoming.delete();}
        },"network-file").start();
    }
}
