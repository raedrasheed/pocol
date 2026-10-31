using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Web.Script.Serialization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;

// Exercise the actual .NET Framework serializer and WPF Response/Update path.
// No engine is launched, key generated, transaction signed or server contacted.
internal static class WalletUiTests {
 const BindingFlags Private=BindingFlags.Instance|BindingFlags.NonPublic;
 static readonly JavaScriptSerializer Json=new JavaScriptSerializer();
 static readonly string Transfer="0x"+new string('a',64),Registration="0x"+new string('b',64);
 static object wallet; static Window window;
 static void Check(bool condition,string message){if(!condition)throw new Exception(message);}
 static T Control<T>(string name) where T:class{return window.FindName(name) as T;}
 static void Invoke(string method,params object[] args){typeof(Wallet).GetMethod(method,Private).Invoke(wallet,args);}
 static void Response(object value){Invoke("Response",Json.Serialize(value));}
 static Dictionary<string,object> D(params object[] pairs){var d=new Dictionary<string,object>();for(int i=0;i<pairs.Length;i+=2)d[(string)pairs[i]]=pairs[i+1];return d;}
 static Dictionary<string,object> Entry(string hash,string kind,string state){return D("hash",hash,"kind",kind,"amount",kind=="registration"?"100":"200.000000000000000001","state",state);}
 static Dictionary<string,object> Snapshot(object activity){return D("type","status","address","0x"+new string('1',40),"balance","1234.000000000000000001","height",143,"chainId",1599846715,"peers",3,"miningEnabled",true,"miningEligible",true,"miningState","Enabled — waiting for assigned work","blocksMined",5,"bond","100","lifetime",D("continuous",true),"activity",activity);}
 static StackPanel Card(int index){var card=Control<StackPanel>("ActivityList").Children[index] as Border;Check(card!=null,"The populated history was not rendered.");return card.Child as StackPanel;}
 static string CardText(int card,int child){return ((TextBlock)Card(card).Children[child]).Text;}
 [STAThread] static int Main(string[] args){
  try{
   string home=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),"PoCol");
   bool homeExisted=Directory.Exists(home);
   var app=new Application{ShutdownMode=ShutdownMode.OnExplicitShutdown};
   wallet=Activator.CreateInstance(typeof(Wallet),true);
   window=(Window)typeof(Wallet).GetField("window",Private).GetValue(wallet);
   var transfer=Entry(Transfer,"transfer","Pending");var registration=Entry(Registration,"registration","Included");registration["confirmations"]=4;
   var state=Snapshot(new object[]{transfer,registration});
   var parsed=Json.Deserialize<Dictionary<string,object>>(Json.Serialize(state));
   Console.WriteLine("Actual nested activity type: "+parsed["activity"].GetType().FullName);
   Response(state);
   Check(Control<StackPanel>("ActivityList").Children.Count==2,"Expected two history rows.");
   Check(CardText(0,0)=="Sent PCOL","Transfer row missing.");
   Check(CardText(0,1)=="200.000000000000000001 PCOL","Amount precision changed.");
   Check(CardText(1,0)=="Mining registration"&&CardText(1,2)=="Included · 4 confirmations","Registration/receipt status missing.");
   Check(((TextBox)Card(0).Children[3]).Text==Transfer,"Transaction hash changed.");
   // Keep newer feature checks after the history regression: the historical
   // baseline has no network-name support and must first fail for missing cards.
   Check(Control<TextBlock>("NetworkName").Text=="●  PoCol network","Unknown networks must not be presented as Mainnet.");
   state["networkName"]="PoCol Mainnet";state["networkEnvironment"]="mainnet";Response(state);
   Check(Control<TextBlock>("NetworkName").Text=="●  PoCol Mainnet","Validated network designation was not displayed.");
   state.Remove("networkName");state.Remove("networkEnvironment");Response(state);
   Check(Control<TextBlock>("NetworkName").Text=="●  PoCol network","The previous network designation was retained for another network.");
   Check(Control<Button>("Upgrade")!=null,"Explicit network upgrade action is missing.");
   Check(Control<TextBlock>("Connection").Text.StartsWith("3 direct peers"),"Peer count must describe direct connections.");
   Check(Control<CheckBox>("MiningToggle").IsChecked==true,"Mining snapshot not reflected.");
   Response(D("type","submitted","hash",Transfer,"historySaved",true));
   Response(state);Check(Control<TextBlock>("Message").Text.StartsWith("Transaction pending"),"Pending status missing.");
   transfer["state"]="Included";transfer["confirmations"]=2;Response(state);
   Check(Control<TextBlock>("Message").Text.StartsWith("Transaction included · 2 confirmations"),"Submitted footer stayed stale after inclusion.");
   transfer["state"]="Reverted";Response(state);
   Check(Control<TextBlock>("Message").Text.StartsWith("Transaction reverted"),"Reverted receipt shown as success.");
   transfer["state"]="Unconfirmed";transfer.Remove("confirmations");Response(state);
   Check(Control<TextBlock>("Message").Text.StartsWith("Transaction unconfirmed"),"Unconfirmed state not restored.");
   Response(state);Check(Control<StackPanel>("ActivityList").Children.Count==2,"Refresh duplicated rows.");
   var pending=typeof(Wallet).GetField("pending",Private);pending.SetValue(wallet,true);Response(state);
   Check((bool)pending.GetValue(wallet),"Periodic status incorrectly acknowledged a pending command.");
   state["type"]="mining";state["miningEnabled"]=false;Response(state);
   Check(!(bool)pending.GetValue(wallet)&&Control<CheckBox>("MiningToggle").IsChecked==false,"Mining acknowledgement not applied.");
   Response(Snapshot(new object[0]));Check(((TextBlock)Control<StackPanel>("ActivityList").Children[0]).Text=="A fresh start","Empty history state missing.");
   Response(Snapshot(null));Check(Control<StackPanel>("ActivityList").Children.Count==2,"Null history failed.");
   Response(Snapshot(new object[]{null,"invalid",registration}));Check(Control<StackPanel>("ActivityList").Children.Count==1&&CardText(0,0)=="Mining registration","Malformed entries were not skipped.");
   transfer["state"]="Included";transfer["confirmations"]=2;state["type"]="status";Response(state);
   if(args.Length==1){
    Invoke("Page","Activity");window.Show();window.UpdateLayout();
    Control<TextBlock>("Message").Text="UI regression fixture · sample transactions, not a live account";
    window.UpdateLayout();var content=(FrameworkElement)window.Content;
    var bitmap=new RenderTargetBitmap((int)Math.Ceiling(content.ActualWidth),(int)Math.Ceiling(content.ActualHeight),96,96,PixelFormats.Pbgra32);bitmap.Render(content);
    var png=new PngBitmapEncoder();png.Frames.Add(BitmapFrame.Create(bitmap));using(var file=File.Create(args[0]))png.Save(file);
   }
   window.Close();app.Shutdown();Check(Directory.Exists(home)==homeExisted,"UI test unexpectedly created a wallet home.");
   Console.WriteLine("PASS actual WPF history rendering, exact amounts, receipt transitions, direct-peer label, mining acknowledgement and refresh");return 0;
  }catch(Exception e){Console.WriteLine("FAIL "+e.GetBaseException().Message);return 1;}
 }
}
