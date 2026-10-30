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
    private static final int BLUE=Color.rgb(26,169,228), INK=Color.rgb(244,248,251), MUTED=Color.rgb(161,177,195), BG=Color.rgb(6,14,24), PANEL=Color.rgb(11,22,35), SOFT=Color.rgb(12,34,51), LINE=Color.rgb(33,49,64);
    private final Handler main=new Handler(Looper.getMainLooper());
    private Messenger service;
    private boolean bound, updating, requestPending;
    private String home,address="",tab="Wallet";
    private long responseId;
    private JSONObject snapshot=new JSONObject();
    private LinearLayout content,nav;
    private TextView balance,connectionStatus,miningStatus,headerStatus;
    private Switch miningSwitch;
    private AlertDialog quoteDialog;
    private final Messenger receiver=new Messenger(new Handler(Looper.getMainLooper(),msg->{
        if(msg.what==1) update(msg.getData().getString("status","{}"));
        if(msg.what==3) try {JSONObject r=new JSONObject(msg.getData().getString("result","{}"));if(r.has("error")){requestPending=false;notice(r.optString("error"));}}catch(JSONException ignored){}
        return true;
    }));
    private final ServiceConnection connection=new ServiceConnection(){
        public void onServiceConnected(ComponentName name,IBinder binder){service=new Messenger(binder);responseId=0;}
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
    @Override public void onCreate(Bundle state){super.onCreate(state);home=new File(getFilesDir(),"pocol").getAbsolutePath();getWindow().setStatusBarColor(BG);getWindow().setNavigationBarColor(BG);getWindow().getDecorView().setSystemUiVisibility(0);
        address=getPreferences(MODE_PRIVATE).getString("address","");
        if(address.isEmpty())try{JSONObject r=new JSONObject(Native.identity(home));address=r.optString("address");if(!address.isEmpty())getPreferences(MODE_PRIVATE).edit().putString("address",address).apply();else notice(r.optString("error","Cannot create identity"));}catch(JSONException e){notice("Cannot create identity");}
        render();
    }
    private void render(){
        balance=null;connectionStatus=null;miningSwitch=null;miningStatus=null;
        LinearLayout root=column();root.setBackgroundColor(BG);root.setPadding(dp(18),dp(12),dp(18),0);
        root.setOnApplyWindowInsetsListener((v,i)->{v.setPadding(dp(18)+i.getSystemWindowInsetLeft(),dp(12)+i.getSystemWindowInsetTop(),dp(18)+i.getSystemWindowInsetRight(),i.getSystemWindowInsetBottom());return i;});setContentView(root);
        LinearLayout top=new LinearLayout(this);top.setGravity(Gravity.CENTER_VERTICAL);root.addView(top,new LinearLayout.LayoutParams(-1,-2));
        ImageView logo=new ImageView(this);logo.setImageResource(R.drawable.pocol_logo);logo.setContentDescription("PoCol Wallet");logo.setScaleType(ImageView.ScaleType.FIT_START);top.addView(logo,new LinearLayout.LayoutParams(0,dp(40),1));
        top.addView(chip("TESTNET",BLUE));
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
        LinearLayout asset=card(content);eyebrow(asset,"NATIVE ASSET");LinearLayout row=new LinearLayout(this);row.setGravity(Gravity.CENTER_VERTICAL);asset.addView(row);ImageView icon=new ImageView(this);icon.setImageResource(R.drawable.pocol_icon);icon.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);LinearLayout.LayoutParams ip=new LinearLayout.LayoutParams(dp(44),dp(44));ip.setMargins(0,0,dp(12),0);row.addView(icon,ip);LinearLayout info=column();row.addView(info,new LinearLayout.LayoutParams(0,-2,1));info.addView(text("PoCol",18,INK,true));info.addView(text("PCOL · Test currency",12,MUTED,false));asset.addView(text("Test coins have no market price.",12,MUTED,false));
        LinearLayout status=card(content);status.addView(text("Connected, on your terms",18,INK,true));connectionStatus=text("Connect your local node to see its verified balance.",14,MUTED,false);status.addView(connectionStatus);button(status,service==null?"Connect wallet":"Manage mining",false,()->{if(service==null)startNode();else{tab="Mining";render();}});
    }
    private void mining(){
        heading("Mining","Contribute when it works for you.");LinearLayout c=card(content);eyebrow(c,"THIS DEVICE");c.addView(text("Device mining",20,INK,true));miningSwitch=new Switch(this);miningSwitch.setText("Enable mining");miningSwitch.setTextSize(16);miningSwitch.setTextColor(INK);miningSwitch.setMinHeight(dp(56));miningSwitch.setPadding(0,dp(12),0,dp(12));miningSwitch.setThumbTintList(new android.content.res.ColorStateList(new int[][]{new int[]{android.R.attr.state_checked},new int[]{}},new int[]{BLUE,MUTED}));miningSwitch.setTrackTintList(new android.content.res.ColorStateList(new int[][]{new int[]{android.R.attr.state_checked},new int[]{}},new int[]{Color.rgb(20,98,134),LINE}));c.addView(miningSwitch);miningStatus=text("Connect your wallet first.",14,MUTED,false);c.addView(miningStatus);
        miningSwitch.setOnCheckedChangeListener((b,on)->{if(!updating){updating=true;b.setChecked(snapshot.optBoolean("miningEnabled"));updating=false;command(object("method","mining","enabled",on));}});
        divider(c);button(c,"Review mining registration",false,()->command(object("method","quote_register")));
        LinearLayout note=card(content);note.addView(text("You stay in control",18,INK,true));note.addView(text("Turning mining off keeps your wallet connected and validating. Turning it on requires an eligible registered account. Registration locks a bond and pays a network fee.",14,MUTED,false));gap(note,10);note.addView(text("Mining uses battery, mobile data and storage. Android may pause the app. Rewards depend on accepted work and are not guaranteed.",13,MUTED,false));
    }
    private void activity(){
        heading("Activity","Transactions submitted from this device");JSONArray items=snapshot.optJSONArray("activity");if(items==null||items.length()==0){LinearLayout c=card(content);c.addView(text("⇄",34,BLUE,false));c.addView(text("Your activity starts here",20,INK,true));c.addView(text("Your transfers and mining registrations will appear here after you submit them.",14,MUTED,false));return;}
        for(int i=0;i<items.length();i++){JSONObject a=items.optJSONObject(i);if(a==null)continue;LinearLayout c=card(content);c.addView(text(a.optString("kind").equals("registration")?"Mining registration":"Sent PCOL",16,INK,true));TextView amount=text(a.optString("amount")+" PCOL",23,INK,true);amount.setTextIsSelectable(true);c.addView(amount);c.addView(chip(a.optString("state"),BLUE),new LinearLayout.LayoutParams(-2,-2));if(a.has("confirmations"))c.addView(text(a.optLong("confirmations")+" confirmations",12,MUTED,false));divider(c);eyebrow(c,"TRANSACTION HASH");TextView hash=text(a.optString("hash"),11,MUTED,false);hash.setTypeface(Typeface.MONOSPACE);hash.setTextIsSelectable(true);c.addView(hash);}
    }
    private void settings(){
        heading("Settings","Your network. Your account. Your device.");LinearLayout c=card(content);eyebrow(c,"CONNECTION");c.addView(text("PoCol network",20,INK,true));c.addView(text("Import the shared network file once. Your private key stays on this device.",14,MUTED,false));button(c,"Import network file",true,()->{if(service!=null){notice("Disconnect the wallet before importing a network.");return;}startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT).setType("application/json").addCategory(Intent.CATEGORY_OPENABLE),1);});button(c,"Connect wallet",false,this::startNode);button(c,"Disconnect wallet",false,()->{if(service!=null)try{service.send(Message.obtain(null,2));headerStatus.setText("Disconnecting…");}catch(RemoteException ignored){}});
        LinearLayout identity=card(content);eyebrow(identity,"PUBLIC ADDRESS");identity.addView(text("Your account",20,INK,true));TextView a=text(address,12,INK,false);a.setTypeface(Typeface.MONOSPACE);a.setTextIsSelectable(true);identity.addView(a);button(identity,"Copy public address",false,this::copy);divider(identity);identity.addView(text("Stored on this device",15,INK,true));identity.addView(text("This test build stores its key on this phone. Uninstalling erases the account and local state. Portable recovery export is not available in this build.",13,MUTED,false));
        TextView version=text("PoCol Wallet · 0.3.0 testnet",12,MUTED,false);version.setGravity(Gravity.CENTER);content.addView(version);
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
    private void review(JSONObject q){
        requestPending=false;String id=q.optString("quoteId");boolean registration=q.optString("kind").equals("registration");LinearLayout body=dialogBody();body.addView(chip("REVIEW BEFORE SIGNING",BLUE),new LinearLayout.LayoutParams(-2,-2));gap(body,14);reviewRow(body,"Network",String.valueOf(q.optLong("chainId")),false);reviewRow(body,"To",q.optString("to"),true);divider(body);reviewRow(body,registration?"Bond":"Amount",q.optString("amount")+" PCOL",false);reviewRow(body,"Maximum fee",q.optString("maximumFee")+" PCOL",false);reviewRow(body,"Maximum total",q.optString("maximumTotal")+" PCOL",false);body.addView(text("Review expires in 60 seconds.",12,MUTED,false));
        quoteDialog=new AlertDialog.Builder(this).setTitle(registration?"Review registration":"Review transfer").setView(scrollBody(body)).setNegativeButton("Cancel",(d,w)->command(object("method","cancel"))).setPositiveButton("Confirm & sign",(d,w)->command(object("method","confirm","quote_id",id))).setOnCancelListener(d->command(object("method","cancel"))).create();quoteDialog.show();styleDialog(quoteDialog);
    }
    private void update(String raw){try{JSONObject v=new JSONObject(raw);if(v.has("address")){snapshot=v;showSnapshot();long id=v.optLong("responseId");if(id>responseId){responseId=id;JSONObject r=v.optJSONObject("response");if(r!=null){requestPending=false;String type=r.optString("type");if(type.equals("quote"))review(r);else if(type.equals("error"))notice(r.optString("message"));else if(type.equals("submitted"))new AlertDialog.Builder(this).setTitle("Transaction submitted").setMessage("Waiting for network inclusion.\n\n"+r.optString("hash")).setPositiveButton("Done",null).show();}}}else if(v.has("error")){headerStatus.setText(v.optString("error"));}else headerStatus.setText("Preparing local validation…");}catch(JSONException ignored){}}
    private void showSnapshot(){if(headerStatus==null)return;if(service==null){headerStatus.setText("Wallet offline · balances unavailable");return;}if(!snapshot.has("balance"))return;if(tab.equals("Activity")){content.removeAllViews();activity();}headerStatus.setText(snapshot.optInt("peers")+" peers · Block "+snapshot.optLong("height"));if(balance!=null)balance.setText(snapshot.optString("balance","—"));if(connectionStatus!=null)connectionStatus.setText(snapshot.optInt("peers")>0?"Your device is validating blocks. Mining is "+(snapshot.optBoolean("miningEnabled")?"on.":"off."):"Looking for peers. Your balance may be out of date.");if(miningSwitch!=null){updating=true;miningSwitch.setChecked(snapshot.optBoolean("miningEnabled"));miningSwitch.setEnabled(snapshot.optBoolean("miningEligible")||snapshot.optBoolean("miningEnabled"));updating=false;}if(miningStatus!=null)miningStatus.setText(snapshot.optString("miningState")+"\nAccepted blocks this session: "+snapshot.optLong("blocksMined")+"\nRegistration bond: "+snapshot.optString("bond")+" PCOL");}
    private void startNode(){if(!new File(home,"network.pocol.json").isFile()){notice("Import the shared network file in Settings first.");tab="Settings";render();return;}if(Build.VERSION.SDK_INT>=33&&checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=PackageManager.PERMISSION_GRANTED)requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS},2);try{startForegroundService(new Intent(this,NodeService.class));attach();headerStatus.setText("Starting local validation…");}catch(RuntimeException e){notice("Cannot start: "+e.getMessage());}}
    private void command(JSONObject command){if(service==null){notice("Connect your wallet first.");return;}if(requestPending){notice("Please wait for the previous request.");return;}Message m=Message.obtain(null,3);Bundle b=new Bundle();b.putString("command",command.toString());m.setData(b);m.replyTo=receiver;try{service.send(m);requestPending=true;main.postDelayed(()->requestPending=false,10000);}catch(RemoteException e){notice("Wallet service unavailable");}}
    private static JSONObject object(Object... pairs){JSONObject j=new JSONObject();try{for(int i=0;i<pairs.length;i+=2)j.put((String)pairs[i],pairs[i+1]);}catch(JSONException ignored){}return j;}
    private String shortAddress(){return address.length()==42?address.substring(0,8)+"…"+address.substring(36):"Account unavailable";}
    private void copy(){getSystemService(android.content.ClipboardManager.class).setPrimaryClip(ClipData.newPlainText("PoCol address",address));notice("Address copied");}
    private void notice(String message){Toast.makeText(this,message,Toast.LENGTH_LONG).show();}
    private void attach(){if(!bound)bound=bindService(new Intent(this,NodeService.class),connection,0);}
    @Override protected void onStart(){super.onStart();attach();main.post(refresh);}
    @Override protected void onStop(){main.removeCallbacks(refresh);if(bound){unbindService(connection);bound=false;service=null;}super.onStop();}
    private static byte[] readLimited(InputStream in)throws IOException{ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] b=new byte[8192];int n;while((n=in.read(b))!=-1){if(out.size()+n>1048576)throw new IOException("File exceeds 1 MiB");out.write(b,0,n);}return out.toByteArray();}
    @Override protected void onActivityResult(int request,int result,Intent data){super.onActivityResult(request,result,data);if(request!=1||result!=RESULT_OK||data==null||data.getData()==null)return;new Thread(()->{File incoming=new File(getCacheDir(),"network-import.json");try(InputStream in=getContentResolver().openInputStream(data.getData())){if(in==null)throw new IOException("Cannot read file");try(FileOutputStream out=new FileOutputStream(incoming)){out.write(readLimited(in));}JSONObject r=new JSONObject(Native.importNetwork(home,incoming.getAbsolutePath()));main.post(()->{if(r.has("error"))notice(r.optString("error"));else{notice("Network imported");startNode();}});}catch(Exception e){main.post(()->notice("Import failed: "+e.getMessage()));}finally{incoming.delete();}},"network-import").start();}
}
