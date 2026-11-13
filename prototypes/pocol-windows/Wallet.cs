using System;
using System.Collections;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using Microsoft.Win32;

[assembly: AssemblyVersion("0.4.0.0")]
[assembly: AssemblyFileVersion("0.4.0.0")]

internal sealed class Wallet {
 readonly Window window; readonly JavaScriptSerializer json=new JavaScriptSerializer { MaxJsonLength=1048576 };
 readonly string folder=AppDomain.CurrentDomain.BaseDirectory;
 readonly string home=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"PoCol");
 Process engine; string address="",lastError="",lastSubmitted=""; bool updating,closing,pending,networkActionPending; Dictionary<string,object> state=new Dictionary<string,object>();
 T Get<T>(string name) where T:class { return window.FindName(name) as T; }
 static string Text(Dictionary<string,object> d,string key){object v;return d.TryGetValue(key,out v)&&v!=null?Convert.ToString(v,System.Globalization.CultureInfo.InvariantCulture):"";}
 static bool Flag(Dictionary<string,object> d,string key){object v;return d.TryGetValue(key,out v)&&v is bool&&(bool)v;}
 static Dictionary<string,object> Cmd(params object[] pairs){var d=new Dictionary<string,object>();for(int i=0;i<pairs.Length;i+=2)d[(string)pairs[i]]=pairs[i+1];return d;}
 void Message(string text){lastSubmitted="";Get<TextBlock>("Message").Text=text;}
 void Hook(string name,Action action){Get<Button>(name).Click+=(s,e)=>action();}
 Wallet(){using(var stream=Assembly.GetExecutingAssembly().GetManifestResourceStream("Wallet.xaml"))window=(Window)XamlReader.Load(stream);
  window.Width=Math.Min(window.Width,Math.Max(window.MinWidth,SystemParameters.WorkArea.Width-32));window.Height=Math.Min(window.Height,Math.Max(window.MinHeight,SystemParameters.WorkArea.Height-32));
  var logo=ImageResource("pocol-logo.png");var icon=ImageResource("pocol-icon.png");
  Get<Image>("BrandLogo").Source=logo;Get<Image>("AssetIcon").Source=icon;Get<Image>("BalanceMark").Source=icon;window.Icon=icon;
  Get<TextBlock>("Connection").ToolTip="Direct connections from this device, not the total number of nodes or miners.";
  foreach(string page in new[]{"Wallet","Activity","Mining","Settings"}){string p=page;Hook("Nav"+p,()=>Page(p));}
  Hook("ManageMining",()=>Page("Mining"));Hook("Connect",Start);Hook("Send",Send);Hook("Receive",Receive);Hook("CopyAddress",Copy);Hook("Import",Import);Hook("Disconnect",()=>Stop());Hook("Register",()=>Command(Cmd("method","quote_register")));Hook("Rejoin",()=>Command(Cmd("method","quote_rejoin")));Hook("TopUp",()=>Command(Cmd("method","quote_top_up")));Hook("Contest",()=>Command(Cmd("method","contest_candidates")));
  Hook("Upgrade",Upgrade);Hook("BackupKey",BackupKey);Hook("RestoreKey",RestoreKey);Get<TextBlock>("BackupInfo").Text=BackupInfo+"\n"+BackupStore;
  Get<CheckBox>("MiningToggle").Click+=(s,e)=>{if(updating)return;bool on=Get<CheckBox>("MiningToggle").IsChecked==true;updating=true;Get<CheckBox>("MiningToggle").IsChecked=Flag(state,"miningEnabled");updating=false;Command(Cmd("method","mining","enabled",on));};
  window.Closing+=async(s,e)=>{if(networkActionPending){e.Cancel=true;Message("Finish the network upgrade before closing the wallet.");return;}if(closing||engine==null||engine.HasExited)return;e.Cancel=true;closing=true;await Stop();window.Close();};
  Page("Wallet");
 }
 static ImageSource ImageResource(string name){using(var stream=Assembly.GetExecutingAssembly().GetManifestResourceStream(name)){if(stream==null)throw new InvalidOperationException("The wallet brand resource is missing: "+name);var bitmap=new BitmapImage();bitmap.BeginInit();bitmap.CacheOption=BitmapCacheOption.OnLoad;bitmap.StreamSource=stream;bitmap.EndInit();bitmap.Freeze();return bitmap;}}
 static SolidColorBrush Brush(string value){return new SolidColorBrush((Color)ColorConverter.ConvertFromString(value));}
 void Page(string page){foreach(string p in new[]{"Wallet","Activity","Mining","Settings"}){bool selected=p==page;Get<StackPanel>(p+"Page").Visibility=selected?Visibility.Visible:Visibility.Collapsed;var button=Get<Button>("Nav"+p);button.Background=Brush(selected?"#0C2233":"#00000000");button.Foreground=Brush(selected?"#73D2FA":"#A1B1C3");button.BorderBrush=Brush(selected?"#214158":"#00000000");}Get<TextBlock>("PageTitle").Text=page;}
 async Task<string> Run(string args){var p=new Process { StartInfo=Info(args) };p.Start();Task<string> stdout=p.StandardOutput.ReadToEndAsync(),stderr=p.StandardError.ReadToEndAsync();await Task.Run(()=>p.WaitForExit());string output=await stdout,error=await stderr;int code=p.ExitCode;p.Dispose();if(code!=0)throw new Exception(error);return output;}
 ProcessStartInfo Info(string args){return new ProcessStartInfo(Path.Combine(folder,"pocol.exe"),args){UseShellExecute=false,CreateNoWindow=true,RedirectStandardInput=true,RedirectStandardOutput=true,RedirectStandardError=true,StandardOutputEncoding=Encoding.UTF8,StandardErrorEncoding=Encoding.UTF8,WorkingDirectory=folder};}
 string HomeArgs(){return " --home \""+home+"\"";}
 async void Initialize(){try{string raw=await Run("identity"+HomeArgs());var d=json.Deserialize<Dictionary<string,object>>(raw);address=Text(d,"address");Get<TextBox>("Address").Text=address;Get<TextBlock>("ShortAddress").Text=address.Length==42?address.Substring(0,8)+"…"+address.Substring(36):address;if(File.Exists(Path.Combine(home,"network.pocol.json"))||File.Exists(Path.Combine(folder,"network.pocol.json")))Start();else Message("Import the shared network file in Settings to connect.");}catch(Exception e){Message(e.Message);}}
 async void Start(){if(networkActionPending){Message("Finish the network upgrade before connecting.");return;}if(engine!=null&&!engine.HasExited)return;pending=false;lastError="";Get<Button>("Connect").IsEnabled=false;Get<TextBlock>("Connection").Text="Preparing local validation…";
  try{engine=new Process {StartInfo=Info("wallet"+HomeArgs()),EnableRaisingEvents=true};Process current=engine;
   current.OutputDataReceived+=(s,e)=>{if(e.Data!=null)window.Dispatcher.BeginInvoke(new Action(()=>Response(e.Data)));};
   current.ErrorDataReceived+=(s,e)=>{if(e.Data!=null)lastError=e.Data.Length>1000?e.Data.Substring(0,1000):e.Data;};
   current.Exited+=(s,e)=>window.Dispatcher.BeginInvoke(new Action(()=>{if(engine!=current)return;Get<Button>("Connect").IsEnabled=true;Get<TextBlock>("Connection").Text="Wallet offline · balances unavailable";Get<TextBlock>("Balance").Text="—";Get<CheckBox>("MiningToggle").IsEnabled=false;pending=false;Message(lastError);}));
   current.Start();current.BeginOutputReadLine();current.BeginErrorReadLine();await Task.Yield();
  }catch(Exception e){Get<Button>("Connect").IsEnabled=true;Message(e.Message);}}
 async Task Stop(){if(engine==null||engine.HasExited)return;Process current=engine;try{current.StandardInput.WriteLine("{\"method\":\"stop\"}");current.StandardInput.Flush();if(await Task.WhenAny(Task.Run(()=>current.WaitForExit()),Task.Delay(10000))!=null&&!current.HasExited)current.Kill();}catch(Exception e){Message(e.Message);}}
 void Command(Dictionary<string,object> command){if(engine==null||engine.HasExited){Message("Connect your wallet first.");return;}if(pending){Message("Please wait for the previous request.");return;}try{engine.StandardInput.WriteLine(json.Serialize(command));engine.StandardInput.Flush();pending=true;}catch(Exception e){pending=false;Message(e.Message);}}
 void Response(string raw){try{var d=json.Deserialize<Dictionary<string,object>>(raw);string type=Text(d,"type");if(type=="status"||type=="mining"){if(type=="mining")pending=false;state=d;Update();return;}if(type=="")return;pending=false;if(type=="error")Message(Text(d,"message"));else if(type=="quote")Review(d);else if(type=="contestCandidates")ChooseContest(d);else if(type=="rejoin")Message(Text(d,"message"));else if(type=="submitted"){Message("Transaction submitted. Waiting for inclusion: "+Text(d,"hash"));lastSubmitted=Text(d,"hash");if(!Flag(d,"historySaved"))MessageBox.Show(window,"The transaction was relayed, but its local history could not be saved. Keep this hash: "+Text(d,"hash"),"History not saved");}else Message(type);}catch(Exception){Message("Could not read a wallet response.");}}
 void Update(){address=Text(state,"address");Get<TextBox>("Address").Text=address;Get<TextBlock>("Balance").Text=Text(state,"balance");Get<TextBlock>("Connection").Text=Text(state,"peers")+" direct peers · Block "+Text(state,"height")+" · Network "+Text(state,"chainId");Get<TextBlock>("WalletNote").Text=Text(state,"peers")=="0"?PeerNote(state):"Your device is validating blocks. Mining is "+(Flag(state,"miningEnabled")?"on.":"off.");Get<TextBlock>("MiningState").Text=Text(state,"miningState");Get<TextBlock>("Mined").Text=MinedText(state);updating=true;Get<CheckBox>("MiningToggle").IsChecked=Flag(state,"miningEnabled");Get<CheckBox>("MiningToggle").IsEnabled=Flag(state,"miningEligible")||Flag(state,"miningEnabled");updating=false;Get<Button>("Rejoin").Visibility=Flag(state,"rejoinAvailable")?Visibility.Visible:Visibility.Collapsed;Get<Button>("TopUp").Visibility=Flag(state,"topUpAvailable")?Visibility.Visible:Visibility.Collapsed;Get<Button>("Contest").Visibility=Flag(state,"contestAvailable")?Visibility.Visible:Visibility.Collapsed;
  ShowNetwork(state);
  object lifetime;if(state.TryGetValue("lifetime",out lifetime)&&lifetime is Dictionary<string,object>){var life=(Dictionary<string,object>)lifetime;Get<TextBlock>("Lifetime").Text=Flag(life,"continuous")?"Continuous network · no scheduled closing block or expiry date":"Final block: "+Text(life,"finalHeight");}
  var list=Get<StackPanel>("ActivityList");list.Children.Clear();
  foreach(var a in Activities(state)){
   string status=Text(a,"state"),hash=Text(a,"hash");
   string detail=status+(a.ContainsKey("confirmations")?" · "+Text(a,"confirmations")+" confirmations":"");
   var panel=new StackPanel();
   string kind=Text(a,"kind");panel.Children.Add(Label(KindLabel(kind),18));
   panel.Children.Add(Label(Text(a,"amount")+" PCOL",24));
   panel.Children.Add(Label(detail,14));
   panel.Children.Add(new TextBox{Text=hash,IsReadOnly=true,FontSize=11,BorderThickness=new Thickness(0),Margin=new Thickness(0,8,0,0)});
   list.Children.Add(new Border{Style=(Style)window.FindResource("Card"),Child=panel});
   if(hash==lastSubmitted){
    // Follow the current canonical receipt; inclusion is not irreversible finality.
    string summary=status=="Included"?"Transaction included":status=="Reverted"?"Transaction reverted":status=="Pending"?"Transaction pending":"Transaction unconfirmed";
    Get<TextBlock>("Message").Text=summary+(a.ContainsKey("confirmations")?" · "+Text(a,"confirmations")+" confirmations":"")+": "+hash;
   }
  }
  if(list.Children.Count==0){list.Children.Add(Label("A fresh start",24));list.Children.Add(Label("Your transfers and registrations will appear here.",14));}
 }
 // While no peer is connected, say why peers were refused (VA-SPEC §2.5, plan §5); the Android wallet shows the same texts.
 static string PeerNote(Dictionary<string,object> s){
  object value;var refused=s.TryGetValue("incompatiblePeers",out value)?value as Dictionary<string,object>:null;
  long consensus,legacy;long.TryParse(refused==null?"":Text(refused,"consensus"),out consensus);long.TryParse(Text(s,"legacyHellos"),out legacy);
  if(consensus>0){string why=Text(s,"lastIncompatibleReason");return "Peers use a different network configuration: apply the published network upgrade file (Settings → Upgrade) or update PoCol."+(why.Length==0?"":"\n"+why);}
  return legacy>0?"Seeds still run PoCol 0.4.0; wait for the announced switch.":"Looking for peers. Your balance may be out of date.";
 }
 // One line per fork entry an inspected upgrade file appends (`pocol inspect-upgrade`).
 static List<string> UpgradeLines(Dictionary<string,object> preview){
  var lines=new List<string>();object value;if(!preview.TryGetValue("appended",out value))return lines;var entries=value as IList;if(entries==null)return lines;
  foreach(object entry in entries){var pair=entry as IList;if(pair==null||pair.Count!=2)continue;string v=Convert.ToString(pair[0],System.Globalization.CultureInfo.InvariantCulture),h=Convert.ToString(pair[1],System.Globalization.CultureInfo.InvariantCulture);lines.Add(v=="2"?"Full mining range at block "+h:"Consensus version "+v+" at block "+h);}
  return lines;
 }
 // The notice for a file already applied (none), or after a saved rebind or activation. Only an activation needs the coordinated release.
 static string UpgradeOutcome(string change){return change=="none"?"Already applied":change=="rebind"?"Release update: consensus rules and schedule unchanged — saved; you can connect now":"Network upgrade saved. Connect your wallet when the coordinated release is ready.";}
 void ShowNetwork(Dictionary<string,object> info){string name=Text(info,"networkName");Get<TextBlock>("NetworkName").Text="●  "+(name.Length==0?"PoCol network":name);}
 // JavaScriptSerializer's untyped nested JSON arrays can be ArrayList, not object[].
 static IEnumerable<Dictionary<string,object>> Activities(Dictionary<string,object> snapshot){
  object value;if(!snapshot.TryGetValue("activity",out value))yield break;
  var entries=value as IList;if(entries==null)yield break;
  foreach(object entry in entries){var item=entry as Dictionary<string,object>;if(item!=null)yield return item;}
 }

 TextBlock Label(string text,double size){return new TextBlock{Text=text,FontSize=size,FontWeight=size>=18?FontWeights.SemiBold:FontWeights.Normal,TextWrapping=TextWrapping.Wrap,LineHeight=size*1.5,Margin=new Thickness(0,4,0,8),Foreground=Brush(size<17?"#A1B1C3":"#F4F8FB")};}
 Window Dialog(string title,StackPanel body){return new Window{Title=title,Owner=window,Icon=window.Icon,WindowStartupLocation=WindowStartupLocation.CenterOwner,Width=520,MaxHeight=Math.Max(400,SystemParameters.WorkArea.Height-80),SizeToContent=SizeToContent.Height,ResizeMode=ResizeMode.NoResize,Background=Brush("#0B1623"),Foreground=Brush("#F4F8FB"),FontFamily=new FontFamily("Segoe UI"),FontSize=14,Resources=window.Resources,UseLayoutRounding=true,Content=new ScrollViewer{VerticalScrollBarVisibility=ScrollBarVisibility.Auto,HorizontalScrollBarVisibility=ScrollBarVisibility.Disabled,Content=new Border{Padding=new Thickness(28),Child=body}}};}
 Button Btn(string title){return new Button{Content=title,Style=(Style)window.FindResource(typeof(Button)),Margin=new Thickness(0,12,0,0)};}
 void Send(){var body=new StackPanel();body.Children.Add(Label("Send PCOL",27));body.Children.Add(Label("Recipient address",13));var to=new TextBox();body.Children.Add(to);body.Children.Add(Label("Amount in PCOL",13));var amount=new TextBox();body.Children.Add(amount);
  // D-V3-21: where the network charges a creation burn, the user may set the maximum fee that bounds gas and burn.
  TextBox bound=null;if(Flag(state,"creationBurn")){body.Children.Add(Label("Maximum fee in PCOL (optional: gas and creation burn together; leave empty for the suggested one)",13));bound=new TextBox();body.Children.Add(bound);}
  body.Children.Add(Label("Review the recipient, amount and maximum fee before signing.",13));var review=Btn("Review transfer");body.Children.Add(review);var dialog=Dialog("Send PCOL",body);review.Click+=(s,e)=>{Command(SendCommand(to.Text.Trim(),amount.Text.Trim(),bound==null?"":bound.Text.Trim()));dialog.Close();};dialog.ShowDialog();}
 // quote_send, with fee_bound only when the user set one (the engine refuses it on a network without a creation burn).
 static Dictionary<string,object> SendCommand(string to,string amount,string bound){var c=Cmd("method","quote_send","to",to,"amount",amount);if(bound.Length>0)c["fee_bound"]=bound;return c;}
 static string KindLabel(string kind){return kind=="registration"?"Mining registration":kind=="rejoin"?"Mining rejoin · exit":kind=="reactivation"?"Mining reactivation":kind=="topup"?"Bond top-up":kind=="contest"?"Mining registration · contest":"Sent PCOL";}
 static IEnumerable<string> Strings(Dictionary<string,object> d,string key){object v;if(!d.TryGetValue(key,out v)||!(v is IEnumerable)||v is string)yield break;foreach(object item in (IEnumerable)v)if(item is string)yield return (string)item;}
 // The mining card's lines: blocks, bond, and on the v3 network the bond still to accrue and the reactivation clock.
 static string MinedText(Dictionary<string,object> s){
  string text="Accepted blocks this session: "+Text(s,"blocksMined")+"\nRegistration bond: "+Text(s,"bond")+" PCOL";
  if(Text(s,"bondRemaining").Length>0)text+="\nBond held: "+Text(s,"bondHeld")+" PCOL · still to accrue: "+Text(s,"bondRemaining")+" PCOL";
  object r;var react=s.TryGetValue("reactivation",out r)?r as Dictionary<string,object>:null;
  if(react!=null&&Text(react,"allowedFrom").Length>0)text+="\nLast reactivation: block "+Text(react,"lastAt")+" · next one allowed from block "+Text(react,"allowedFrom");
  return text;
 }
 // D-V3-21: the creation burn apart from the gas fee and the amount, and the signed maximum that bounds both.
 static List<string> BurnLines(Dictionary<string,object> q){
  var lines=new List<string>();if(!q.ContainsKey("creationBurn"))return lines;
  lines.Add("Creation burn (quoted): "+Text(q,"creationBurn")+" PCOL");lines.Add("Gas fee (at most): "+Text(q,"gasFee")+" PCOL");
  lines.Add("Maximum fee you sign: "+Text(q,"feeBound")+" PCOL · it bounds the gas and the burn together"+(Flag(q,"feeBoundSet")?" (your maximum)":" (suggested)"));
  if(Flag(q,"feeBoundSet")&&Text(q,"suggestedFeeBound")!=Text(q,"feeBound"))lines.Add("Suggested maximum fee: "+Text(q,"suggestedFeeBound")+" PCOL");
  foreach(string note in Strings(q,"burnNote"))lines.Add(note);
  return lines;
 }
 // V3-10/V3-14: what a contest review states.
 static List<string> ContestLines(Dictionary<string,object> q){
  var lines=new List<string>();object c;var contest=q.TryGetValue("contest",out c)?c as Dictionary<string,object>:null;
  lines.Add((contest!=null&&Text(contest,"kind")=="pool"?"Probation-pool contest of ":"Seat contest of ")+Text(q,"victim"));
  int i=1;foreach(string step in Strings(q,"steps")){lines.Add(i+". "+step);i++;}
  lines.Add("Bond now: "+Text(q,"amount")+" PCOL"+(Flag(q,"probation")?" · filled from your rewards up to "+Text(q,"bondTarget")+" PCOL":""));
  if(Text(q,"note").Length>0)lines.Add(Text(q,"note"));
  return lines;
 }
 // One line per contest candidate (`contest_candidates`).
 static List<string> CandidateLines(Dictionary<string,object> d,string key){
  var lines=new List<string>();object v;if(!d.TryGetValue(key,out v))return lines;var items=v as IList;if(items==null)return lines;
  foreach(object item in items){var c=item as Dictionary<string,object>;if(c==null)continue;lines.Add(Text(c,"address")+" · "+Text(c,"count")+" shares, threshold "+Text(c,"threshold"));}
  return lines;
 }
 void ChooseContest(Dictionary<string,object> d){
  var body=new StackPanel();body.Children.Add(Label("Contest a seat",27));
  if(Text(d,"message").Length>0)body.Children.Add(Label(Text(d,"message"),14));
  var victim=new TextBox{FontFamily=new FontFamily("Consolas"),FontSize=13};
  foreach(string key in new[]{"seat","pool"}){var lines=CandidateLines(d,key);if(lines.Count==0)continue;
   body.Children.Add(Label(key=="seat"?"Members you can contest with the full bond":"Probationers you can contest with a probation bond (every probation seat is taken)",13));
   foreach(string line in lines){string address=line.Substring(0,Math.Min(42,line.Length));var pick=new Button{Content=line,Style=(Style)window.FindResource("Secondary"),HorizontalContentAlignment=HorizontalAlignment.Left,Margin=new Thickness(0,6,0,0),FontSize=12};pick.Click+=(s,e)=>victim.Text=address;body.Children.Add(pick);}}
  body.Children.Add(Label("Member address",13));body.Children.Add(victim);
  body.Children.Add(Label("Bond now in PCOL (optional: empty or 0 is a probation bond filled from your rewards)",13));var bond=new TextBox();body.Children.Add(bond);
  body.Children.Add(Label(Text(d,"note"),13));var review=Btn("Review contest");body.Children.Add(review);var dialog=Dialog("Contest a seat",body);
  review.Click+=(s,e)=>{var c=Cmd("method","quote_contest","victim",victim.Text.Trim());if(bond.Text.Trim().Length>0)c["bond"]=bond.Text.Trim();dialog.Close();Command(c);};dialog.ShowDialog();
 }
 void Review(Dictionary<string,object> q){string kind=Text(q,"kind");if(kind=="rejoin"||kind=="reactivation"){ReviewRejoin(q);return;}var body=new StackPanel();bool reg=kind=="registration",top=kind=="topup",contest=kind=="contest";body.Children.Add(Label(reg?"Review registration":top?"Review bond top-up":contest?"Review contest":"Review transfer",26));
  if(contest){foreach(string line in ContestLines(q))body.Children.Add(Label(line,14));}body.Children.Add(Label("Network "+Text(q,"chainId"),13));body.Children.Add(Label("To",13));body.Children.Add(new TextBox{Text=Text(q,"to"),IsReadOnly=true,FontSize=13});if(!contest)body.Children.Add(Label((reg?"Bond now: ":top?"Top-up: ":"Amount: ")+Text(q,"amount")+" PCOL",19));
  if(reg&&Flag(q,"probation"))body.Children.Add(Label("Probation: the rest of the "+Text(q,"bondTarget")+" PCOL bond is filled from your mining rewards. You can top it up any time.",13));
  if(top)body.Children.Add(Label("Bond held now: "+Text(q,"bondHeld")+" of "+Text(q,"bondTarget")+" PCOL. If rewards fill part of it first, the extra top-up returns to you.",13));
  if(top&&Text(q,"note").Length>0)body.Children.Add(Label(Text(q,"note"),13));
  var burn=BurnLines(q);for(int i=0;i<burn.Count;i++)body.Children.Add(Label(burn[i],i<3?15:13));
  if(burn.Count==0)body.Children.Add(Label("Maximum fee: "+Text(q,"maximumFee")+" PCOL",14));body.Children.Add(Label("Maximum total: "+Text(q,"maximumTotal")+" PCOL",17));body.Children.Add(Label("Review expires in 60 seconds. Actual fees can be lower.",13));
  // D-V3-21: the user lowers or raises the maximum fee and reviews again; nothing is signed until Confirm.
  TextBox bound=null;Button again=null;if(burn.Count>0&&kind=="transfer"){body.Children.Add(Label("Change the maximum fee (PCOL)",13));bound=new TextBox{Text=Text(q,"feeBound")};body.Children.Add(bound);again=Btn("Review with this maximum fee");again.Style=(Style)window.FindResource("Secondary");body.Children.Add(again);}
  var button=Btn("Confirm & sign");body.Children.Add(button);var dialog=Dialog("Review transaction",body);bool confirmed=false,requote=false;button.Click+=(s,e)=>{confirmed=true;Command(Cmd("method","confirm","quote_id",Text(q,"quoteId")));dialog.Close();};
  if(again!=null)again.Click+=(s,e)=>{requote=true;dialog.Close();};
  dialog.ShowDialog();if(requote){Command(SendCommand(Text(q,"to"),Text(q,"amount"),bound.Text.Trim()));return;}if(!confirmed)Command(Cmd("method","cancel"));}
 void ReviewRejoin(Dictionary<string,object> q){var body=new StackPanel();body.Children.Add(Label(Text(q,"kind")=="rejoin"?"Review mining rejoin":"Review mining reactivation",26));body.Children.Add(Label("Network "+Text(q,"chainId"),13));body.Children.Add(Label("Registry",13));body.Children.Add(new TextBox{Text=Text(q,"to"),IsReadOnly=true,FontSize=13});
  object steps;if(q.TryGetValue("steps",out steps)&&steps is IEnumerable&&!(steps is string)){int i=1;foreach(object step in (IEnumerable)steps){if(step is string){body.Children.Add(Label(i+". "+(string)step,14));i++;}}}
  body.Children.Add(Label("Transactions signed by this wallet: up to "+Text(q,"transactions"),14));body.Children.Add(Label("Amount: 0 PCOL · the bond stays locked",17));body.Children.Add(Label("Maximum fees: "+Text(q,"maximumFee")+" PCOL",17));body.Children.Add(Label("Review expires in 60 seconds. Actual fees can be lower.",13));
  var button=Btn("Confirm & start rejoin");body.Children.Add(button);var dialog=Dialog("Review mining rejoin",body);bool confirmed=false;button.Click+=(s,e)=>{confirmed=true;Command(Cmd("method","confirm","quote_id",Text(q,"quoteId")));dialog.Close();};dialog.ShowDialog();if(!confirmed)Command(Cmd("method","cancel"));}
 void Receive(){var body=new StackPanel();body.Children.Add(Label("Receive PCOL",27));body.Children.Add(Label("Use this address on the same PoCol network only.",14));body.Children.Add(new TextBox{Text=address,IsReadOnly=true,FontSize=14,TextWrapping=TextWrapping.Wrap});var copy=Btn("Copy public address");copy.Click+=(s,e)=>Copy();body.Children.Add(copy);Dialog("Receive PCOL",body).ShowDialog();}
 void Copy(){try{Clipboard.SetText(address);Message("Public address copied.");}catch(Exception e){Message(e.Message);}}
 async void Import(){if(networkActionPending){Message("Finish the network upgrade first.");return;}if(engine!=null&&!engine.HasExited){Message("Disconnect the wallet before importing a network.");return;}var pick=new OpenFileDialog{Filter="PoCol network (*.json)|*.json",CheckFileExists=true};if(pick.ShowDialog(window)!=true)return;try{await Run("import"+HomeArgs()+" --network \""+pick.FileName+"\"");Message("Network imported.");Start();}catch(Exception e){Message(e.Message);}}
 async void Upgrade(){
  if(networkActionPending){Message("Finish the current network upgrade first.");return;}
  if(engine!=null&&!engine.HasExited){Message("Disconnect the wallet before upgrading its network.");return;}
  if(!File.Exists(Path.Combine(home,"network.pocol.json"))){Message("Import a network before upgrading it.");return;}
  var pick=new OpenFileDialog{Title="Select the shared network upgrade file",Filter="PoCol network (*.json)|*.json",CheckFileExists=true};if(pick.ShowDialog(window)!=true)return;
  string incoming=null;networkActionPending=true;Get<Button>("Connect").IsEnabled=false;
  try{
   // Inspect and apply the same bounded public-file snapshot shown for confirmation.
   incoming=Path.GetTempFileName();
   using(var source=File.OpenRead(pick.FileName))using(var target=File.Open(incoming,FileMode.Truncate,FileAccess.Write,FileShare.None)){
    var buffer=new byte[8192];int count,total=0;while((count=source.Read(buffer,0,buffer.Length))!=0){total+=count;if(total>1048576)throw new InvalidOperationException("Network file exceeds 1 MiB.");target.Write(buffer,0,count);}target.Flush(true);
   }
   string networkArg=" --network \""+incoming+"\"";
   // A read-only dry run against this home: none, a rebind to a new release of the same rules, or appended fork entries.
   var preview=json.Deserialize<Dictionary<string,object>>(await Run("inspect-upgrade"+HomeArgs()+networkArg));
   string change=Text(preview,"change");if(change=="none"){Message(UpgradeOutcome(change));return;}
   var body=new StackPanel();body.Children.Add(Label(change=="rebind"?"Release update":"Upgrade existing network",26));
   string name=Text(preview,"networkName");body.Children.Add(Label(name.Length==0?"PoCol network":name,19));
   body.Children.Add(Label("Chain ID: "+Text(preview,"chainId"),14));foreach(string line in UpgradeLines(preview))body.Children.Add(Label(line,18));
   body.Children.Add(Label("Genesis hash",13));body.Children.Add(new TextBox{Text=Text(preview,"genesisHash"),IsReadOnly=true,FontSize=12,TextWrapping=TextWrapping.Wrap});
   body.Children.Add(Label(change=="rebind"?"This release runs the same consensus rules and fork schedule. Saving the new file changes no rule. Your account, encrypted key and stored chain stay in the same Windows profile.":"Use the shared upgrade file supplied for this network. The node checks it against your existing network and stored blocks. Your account, encrypted key and stored chain stay in the same Windows profile.",14));
   bool confirmed=false;var confirm=Btn("Confirm network upgrade");body.Children.Add(confirm);var cancel=Btn("Cancel");body.Children.Add(cancel);var dialog=Dialog("Review network upgrade",body);
   confirm.Click+=(s,e)=>{confirmed=true;dialog.Close();};cancel.Click+=(s,e)=>dialog.Close();dialog.ShowDialog();if(!confirmed)return;
   var result=json.Deserialize<Dictionary<string,object>>(await Run("upgrade"+HomeArgs()+networkArg));ShowNetwork(result);
   Message(UpgradeOutcome(change));
  }catch(Exception e){Message(e.Message);}
  finally{if(incoming!=null)try{File.Delete(incoming);}catch(IOException){}catch(UnauthorizedAccessException){}networkActionPending=false;Get<Button>("Connect").IsEnabled=true;}
 }
 // Key backup (CRY-02): pocol export-key / inspect-key-backup / import-key, the format of the Android wallet
 // (pocold keyfile::backup). The passphrase goes to pocol.exe as UTF-8 on stdin, never on the command line. It never
 // becomes a .NET string: PasswordBox.SecurePassword is copied out of unmanaged memory into a char[] and encoded into a
 // byte[], and every copy is zeroed after use. Best effort: the PasswordBox and the input method keep their own copies.
 const string BackupInfo="Back up your key to an encrypted file. Anyone with the file and its passphrase controls your PCOL. Without a backup, a lost device, a reset or an uninstall loses the account for good.";
 const string BackupPassphrase="Choose a passphrase of at least 12 characters that is not a repeated pattern, a sequence or a common password. It is never stored: if you forget it, the backup cannot be opened.";
 const string BackupStore="Keep the file and the passphrase in different places. Never share either one, and never send them to anyone who asks.";
 const string BackupSaved="Encrypted key backup saved. Keep it and its passphrase apart.";
 const string RestoreReplace="Restoring replaces the account this wallet uses now. Its key file is kept in the wallet folder, but move its funds first if you have no backup of it.";
 const string RestoreMining="Stop mining on any other device that uses the restored key: two devices mining with one key can lose the bond.";
 const string KeyDisconnect="Disconnect the wallet before backing up or restoring its key.";
 const string PassphraseMismatch="The passphrases differ. Nothing was saved.";
 static string Restored(bool changed,string account){return changed?"Key restored. This wallet now uses account "+account+".":"This backup holds the account this wallet already uses.";}
 bool keyActionPending;
 static byte[] Utf8(System.Security.SecureString secret){
  IntPtr raw=IntPtr.Zero;char[] chars=new char[secret.Length];
  try{
   raw=System.Runtime.InteropServices.Marshal.SecureStringToGlobalAllocUnicode(secret);
   for(int i=0;i<chars.Length;i++)chars[i]=(char)System.Runtime.InteropServices.Marshal.ReadInt16(raw,2*i);
   return new UTF8Encoding(false,true).GetBytes(chars);
  }finally{Array.Clear(chars,0,chars.Length);if(raw!=IntPtr.Zero)System.Runtime.InteropServices.Marshal.ZeroFreeGlobalAllocUnicode(raw);}
 }
 // The box's passphrase as UTF-8; an unpaired surrogate throws ArgumentException (EncoderFallbackException).
 static byte[] Passphrase(PasswordBox box){using(System.Security.SecureString s=box.SecurePassword)return Utf8(s);}
 static bool Same(byte[] a,byte[] b){if(a.Length!=b.Length)return false;int d=0;for(int i=0;i<a.Length;i++)d|=a[i]^b[i];return d==0;}
 static void Wipe(byte[] b){if(b!=null)Array.Clear(b,0,b.Length);}
 // `times` stdin lines of the passphrase, each ended by a line feed.
 static byte[] Lines(byte[] pass,int times){var lines=new byte[(pass.Length+1)*times];for(int t=0;t<times;t++){Buffer.BlockCopy(pass,0,lines,t*(pass.Length+1),pass.Length);lines[t*(pass.Length+1)+pass.Length]=(byte)'\n';}return lines;}
 [System.Runtime.InteropServices.DllImport("user32.dll")] static extern bool SetWindowDisplayAffinity(IntPtr window,uint affinity);
 // The counterpart of Android's FLAG_SECURE: Windows 10 2004 and later leave the window out of screenshots and screen
 // recordings (WDA_EXCLUDEFROMCAPTURE); older versions show it black instead (WDA_MONITOR).
 static void Protect(Window dialog){dialog.SourceInitialized+=(s,e)=>{try{IntPtr h=new System.Windows.Interop.WindowInteropHelper(dialog).Handle;if(!SetWindowDisplayAffinity(h,0x11))SetWindowDisplayAffinity(h,0x01);}catch(Exception){}};}
 async Task<string> RunWithInput(string args,byte[] input){
  var p=new Process { StartInfo=Info(args) };
  try{
   p.Start();Task<string> stdout=p.StandardOutput.ReadToEndAsync(),stderr=p.StandardError.ReadToEndAsync();
   // pocol.exe can exit before it reads stdin (an existing --out file, a locked home). The pipe is closed then, and
   // pocol's own message on stderr is the error to show, not "The pipe has been ended".
   try{p.StandardInput.BaseStream.Write(input,0,input.Length);p.StandardInput.BaseStream.Flush();}catch(IOException){}
   try{p.StandardInput.Close();}catch(IOException){}
   await Task.Run(()=>p.WaitForExit());string output=await stdout,error=(await stderr).Trim();
   if(p.ExitCode!=0)throw new Exception(error.Length>0?error:"pocol.exe exited with code "+p.ExitCode);
   return output;
  }finally{p.Dispose();}
 }
 bool KeyActionAllowed(){if(engine!=null&&!engine.HasExited){Message(KeyDisconnect);return false;}if(keyActionPending||networkActionPending){Message("Finish the current action first.");return false;}return true;}
 async void BackupKey(){
  if(!KeyActionAllowed())return;
  var body=new StackPanel();body.Children.Add(Label("Back up your key",26));body.Children.Add(Label(BackupInfo,14));body.Children.Add(Label(BackupPassphrase,13));body.Children.Add(Label(BackupStore,13));
  body.Children.Add(Label("Backup passphrase",13));var first=new PasswordBox();body.Children.Add(first);body.Children.Add(Label("Repeat the passphrase",13));var second=new PasswordBox();body.Children.Add(second);
  var ok=Btn("Encrypt backup");body.Children.Add(ok);var dialog=Dialog("Back up your key",body);Protect(dialog);byte[] passphrase=null;
  ok.Click+=(s,e)=>{
   byte[] a=null,b=null;
   try{a=Passphrase(first);b=Passphrase(second);}catch(ArgumentException){Wipe(a);Wipe(b);Message("This passphrase cannot be used; choose another one.");return;}
   if(!Same(a,b)){Wipe(a);Wipe(b);Message(PassphraseMismatch);return;}
   Wipe(b);Wipe(passphrase);passphrase=a;dialog.Close();
  };
  dialog.ShowDialog();first.Clear();second.Clear();if(passphrase==null)return;
  byte[] input=null;
  try{
   var save=new SaveFileDialog{Title="Save the encrypted key backup",Filter="PoCol key backup (*.pocol-backup)|*.pocol-backup",FileName="pocol-key-"+(address.Length>=10?address.Substring(0,10):"wallet")+".pocol-backup",OverwritePrompt=false};
   if(save.ShowDialog(window)!=true)return;
   keyActionPending=true;Get<Button>("Connect").IsEnabled=false;Message("Encrypting your key backup…");
   input=Lines(passphrase,2);
   await RunWithInput("export-key"+HomeArgs()+" --out \""+save.FileName+"\"",input);Message(BackupSaved);
  }
  catch(Exception e){Message(e.Message);}
  finally{Wipe(passphrase);Wipe(input);keyActionPending=false;Get<Button>("Connect").IsEnabled=true;}
 }
 async void RestoreKey(){
  if(!KeyActionAllowed())return;
  var pick=new OpenFileDialog{Title="Select an encrypted key backup",Filter="PoCol key backup (*.pocol-backup)|*.pocol-backup|All files (*.*)|*.*",CheckFileExists=true};if(pick.ShowDialog(window)!=true)return;
  var body=new StackPanel();body.Children.Add(Label("Restore key from backup",26));body.Children.Add(Label("Enter the passphrase chosen when this backup was made.",14));
  body.Children.Add(Label("Backup passphrase",13));var box=new PasswordBox();body.Children.Add(box);var open=Btn("Open backup");body.Children.Add(open);var dialog=Dialog("Restore key from backup",body);Protect(dialog);byte[] passphrase=null;
  open.Click+=(s,e)=>{
   byte[] p=null;
   try{p=Passphrase(box);}catch(ArgumentException){Message("This passphrase cannot be used.");return;}
   if(p.Length==0){Wipe(p);return;}
   Wipe(passphrase);passphrase=p;dialog.Close();
  };
  dialog.ShowDialog();box.Clear();if(passphrase==null)return;
  string backup=" --backup \""+pick.FileName+"\"";
  byte[] input=Lines(passphrase,1);
  keyActionPending=true;Get<Button>("Connect").IsEnabled=false;Message("Opening backup…");
  try{
   var preview=json.Deserialize<Dictionary<string,object>>(await RunWithInput("inspect-key-backup"+HomeArgs()+backup,input));
   string current=Text(preview,"current"),replace="";
   if(Flag(preview,"replaces")){
    var review=new StackPanel();review.Children.Add(Label("Replace this wallet's account?",26));
    review.Children.Add(Label("Current account",13));review.Children.Add(new TextBox{Text=current,IsReadOnly=true,FontSize=13,FontFamily=new FontFamily("Consolas")});
    review.Children.Add(Label("Restored account",13));review.Children.Add(new TextBox{Text=Text(preview,"address"),IsReadOnly=true,FontSize=13,FontFamily=new FontFamily("Consolas")});
    review.Children.Add(Label(RestoreReplace,13));review.Children.Add(Label(RestoreMining,13));
    bool confirmed=false;var confirm=Btn("Replace account");review.Children.Add(confirm);var cancel=Btn("Cancel");review.Children.Add(cancel);var reviewDialog=Dialog("Replace this wallet's account?",review);Protect(reviewDialog);
    confirm.Click+=(s,e)=>{confirmed=true;reviewDialog.Close();};cancel.Click+=(s,e)=>reviewDialog.Close();reviewDialog.ShowDialog();
    if(!confirmed){Message("Restore cancelled. Nothing changed.");return;}
    // The address the user confirmed (0x and 40 hex digits from pocol itself) is the only value put on the command line.
    if(current.Length!=42||!current.StartsWith("0x")){Message("Unexpected current account.");return;}
    replace=" --replace "+current;
   }
   var result=json.Deserialize<Dictionary<string,object>>(await RunWithInput("import-key"+HomeArgs()+backup+replace,input));
   address=Text(result,"address");Get<TextBox>("Address").Text=address;Get<TextBlock>("ShortAddress").Text=address.Length==42?address.Substring(0,8)+"…"+address.Substring(36):address;
   Message(Restored(Flag(result,"changed"),address));
  }catch(Exception e){Message(e.Message);}
  finally{Wipe(passphrase);Wipe(input);keyActionPending=false;Get<Button>("Connect").IsEnabled=true;}
 }
 [STAThread] static void Main(string[] args){var app=new Application();var wallet=new Wallet();if(args.Length==2&&args[0]=="--preview"){wallet.window.Show();wallet.window.UpdateLayout();wallet.Get<TextBlock>("Message").Text="Design preview · no account balance or live connection";wallet.window.UpdateLayout();var bitmap=new RenderTargetBitmap((int)wallet.window.ActualWidth,(int)wallet.window.ActualHeight,96,96,PixelFormats.Pbgra32);bitmap.Render(wallet.window);var encoder=new PngBitmapEncoder();encoder.Frames.Add(BitmapFrame.Create(bitmap));using(var file=File.Create(args[1]))encoder.Save(file);wallet.window.Close();return;}wallet.window.Loaded+=(s,e)=>wallet.Initialize();app.Run(wallet.window);}
}
