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
  Hook("ManageMining",()=>Page("Mining"));Hook("Connect",Start);Hook("Send",Send);Hook("Receive",Receive);Hook("CopyAddress",Copy);Hook("Import",Import);Hook("Disconnect",()=>Stop());Hook("Register",()=>Command(Cmd("method","quote_register")));
  Hook("Upgrade",Upgrade);
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
 void Response(string raw){try{var d=json.Deserialize<Dictionary<string,object>>(raw);string type=Text(d,"type");if(type=="status"||type=="mining"){if(type=="mining")pending=false;state=d;Update();return;}if(type=="")return;pending=false;if(type=="error")Message(Text(d,"message"));else if(type=="quote")Review(d);else if(type=="submitted"){Message("Transaction submitted. Waiting for inclusion: "+Text(d,"hash"));lastSubmitted=Text(d,"hash");if(!Flag(d,"historySaved"))MessageBox.Show(window,"The transaction was relayed, but its local history could not be saved. Keep this hash: "+Text(d,"hash"),"History not saved");}else Message(type);}catch(Exception){Message("Could not read a wallet response.");}}
 void Update(){address=Text(state,"address");Get<TextBox>("Address").Text=address;Get<TextBlock>("Balance").Text=Text(state,"balance");Get<TextBlock>("Connection").Text=Text(state,"peers")+" direct peers · Block "+Text(state,"height")+" · Network "+Text(state,"chainId");Get<TextBlock>("WalletNote").Text=Text(state,"peers")=="0"?"Looking for peers. Your balance may be out of date.":"Your device is validating blocks. Mining is "+(Flag(state,"miningEnabled")?"on.":"off.");Get<TextBlock>("MiningState").Text=Text(state,"miningState");Get<TextBlock>("Mined").Text="Accepted blocks this session: "+Text(state,"blocksMined")+"\nRegistration bond: "+Text(state,"bond")+" PCOL";updating=true;Get<CheckBox>("MiningToggle").IsChecked=Flag(state,"miningEnabled");Get<CheckBox>("MiningToggle").IsEnabled=Flag(state,"miningEligible")||Flag(state,"miningEnabled");updating=false;
  ShowNetwork(state);
  object lifetime;if(state.TryGetValue("lifetime",out lifetime)&&lifetime is Dictionary<string,object>){var life=(Dictionary<string,object>)lifetime;Get<TextBlock>("Lifetime").Text=Flag(life,"continuous")?"Continuous network · no scheduled closing block or expiry date":"Final block: "+Text(life,"finalHeight");}
  var list=Get<StackPanel>("ActivityList");list.Children.Clear();
  foreach(var a in Activities(state)){
   string status=Text(a,"state"),hash=Text(a,"hash");
   string detail=status+(a.ContainsKey("confirmations")?" · "+Text(a,"confirmations")+" confirmations":"");
   var panel=new StackPanel();
   panel.Children.Add(Label(Text(a,"kind")=="registration"?"Mining registration":"Sent PCOL",18));
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
 void Send(){var body=new StackPanel();body.Children.Add(Label("Send PCOL",27));body.Children.Add(Label("Recipient address",13));var to=new TextBox();body.Children.Add(to);body.Children.Add(Label("Amount in PCOL",13));var amount=new TextBox();body.Children.Add(amount);body.Children.Add(Label("Review the recipient, amount and maximum fee before signing.",13));var review=Btn("Review transfer");body.Children.Add(review);var dialog=Dialog("Send PCOL",body);review.Click+=(s,e)=>{Command(Cmd("method","quote_send","to",to.Text.Trim(),"amount",amount.Text.Trim()));dialog.Close();};dialog.ShowDialog();}
 void Review(Dictionary<string,object> q){var body=new StackPanel();bool reg=Text(q,"kind")=="registration";body.Children.Add(Label(reg?"Review registration":"Review transfer",26));body.Children.Add(Label("Network "+Text(q,"chainId"),13));body.Children.Add(Label("To",13));body.Children.Add(new TextBox{Text=Text(q,"to"),IsReadOnly=true,FontSize=13});body.Children.Add(Label((reg?"Bond: ":"Amount: ")+Text(q,"amount")+" PCOL",19));body.Children.Add(Label("Maximum fee: "+Text(q,"maximumFee")+" PCOL",14));body.Children.Add(Label("Maximum total: "+Text(q,"maximumTotal")+" PCOL",17));body.Children.Add(Label("Review expires in 60 seconds. Actual fees can be lower.",13));var button=Btn("Confirm & sign");body.Children.Add(button);var dialog=Dialog("Review transaction",body);bool confirmed=false;button.Click+=(s,e)=>{confirmed=true;Command(Cmd("method","confirm","quote_id",Text(q,"quoteId")));dialog.Close();};dialog.ShowDialog();if(!confirmed)Command(Cmd("method","cancel"));}
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
   var preview=json.Deserialize<Dictionary<string,object>>(await Run("inspect"+networkArg));
   string activation=Text(preview,"fullNonceActivationHeight");if(activation.Length==0)throw new InvalidOperationException("Select a network file with a full-range activation block.");
   var body=new StackPanel();body.Children.Add(Label("Upgrade existing network",26));
   string name=Text(preview,"networkName");body.Children.Add(Label(name.Length==0?"PoCol network":name,19));
   body.Children.Add(Label("Chain ID: "+Text(preview,"chainId"),14));body.Children.Add(Label("Full mining range at block "+activation,18));
   body.Children.Add(Label("Genesis hash",13));body.Children.Add(new TextBox{Text=Text(preview,"genesisHash"),IsReadOnly=true,FontSize=12,TextWrapping=TextWrapping.Wrap});
   body.Children.Add(Label("Use the shared upgrade file supplied for this network. The node checks it against your existing network and stored blocks. Your account, encrypted key and stored chain stay in the same Windows profile.",14));
   bool confirmed=false;var confirm=Btn("Confirm network upgrade");body.Children.Add(confirm);var cancel=Btn("Cancel");body.Children.Add(cancel);var dialog=Dialog("Review network upgrade",body);
   confirm.Click+=(s,e)=>{confirmed=true;dialog.Close();};cancel.Click+=(s,e)=>dialog.Close();dialog.ShowDialog();if(!confirmed)return;
   var result=json.Deserialize<Dictionary<string,object>>(await Run("upgrade"+HomeArgs()+networkArg));ShowNetwork(result);
   Message("Network upgrade saved. Connect your wallet when the coordinated release is ready.");
  }catch(Exception e){Message(e.Message);}
  finally{if(incoming!=null)try{File.Delete(incoming);}catch(IOException){}catch(UnauthorizedAccessException){}networkActionPending=false;Get<Button>("Connect").IsEnabled=true;}
 }
 [STAThread] static void Main(string[] args){var app=new Application();var wallet=new Wallet();if(args.Length==2&&args[0]=="--preview"){wallet.window.Show();wallet.window.UpdateLayout();wallet.Get<TextBlock>("Message").Text="Design preview · no account balance or live connection";wallet.window.UpdateLayout();var bitmap=new RenderTargetBitmap((int)wallet.window.ActualWidth,(int)wallet.window.ActualHeight,96,96,PixelFormats.Pbgra32);bitmap.Render(wallet.window);var encoder=new PngBitmapEncoder();encoder.Frames.Add(BitmapFrame.Create(bitmap));using(var file=File.Create(args[1]))encoder.Save(file);wallet.window.Close();return;}wallet.window.Loaded+=(s,e)=>wallet.Initialize();app.Run(wallet.window);}
}
