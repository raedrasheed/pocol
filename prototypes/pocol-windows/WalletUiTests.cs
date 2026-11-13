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
   // Rejoin mining: the engine decides availability; the UI shows the action, progress and history.
   state["type"]="status";Response(state);Check(Control<Button>("Rejoin").Visibility==Visibility.Collapsed,"Rejoin shown although the engine did not offer it.");
   state["rejoinAvailable"]=true;Response(state);Check(Control<Button>("Rejoin").Visibility==Visibility.Visible,"Rejoin action missing for an inactive member.");
   state["rejoinAvailable"]=false;Response(state);Check(Control<Button>("Rejoin").Visibility==Visibility.Collapsed,"Rejoin stayed visible after it became unavailable.");
   Response(D("type","rejoin","state","Reactivating","message","Reactivation submitted (attempt 1 of 3)."));Check(Control<TextBlock>("Message").Text=="Reactivation submitted (attempt 1 of 3).","Rejoin progress not shown.");
   Response(Snapshot(new object[]{Entry(Transfer,"reactivation","Included"),Entry(Registration,"rejoin","Included")}));Check(CardText(0,0)=="Mining reactivation"&&CardText(1,0)=="Mining rejoin · exit","Rejoin history rows missing.");
   // Earn-in probation: the engine offers a top-up; the UI shows the action and its history row.
   state["type"]="status";state.Remove("rejoinAvailable");Response(state);Check(Control<Button>("TopUp").Visibility==Visibility.Collapsed,"Top-up shown although the engine did not offer it.");
   state["topUpAvailable"]=true;Response(state);Check(Control<Button>("TopUp").Visibility==Visibility.Visible,"Top-up action missing during probation.");
   state["topUpAvailable"]=false;Response(state);Check(Control<Button>("TopUp").Visibility==Visibility.Collapsed,"Top-up stayed visible after probation.");
   Response(Snapshot(new object[]{Entry(Transfer,"topup","Included")}));Check(CardText(0,0)=="Bond top-up","Top-up history row missing.");
   // V3 (the v3 program set): the contest action, the probation bond still to accrue, the reactivation clock and the
   // contest history row; the engine decides availability.
   state["type"]="status";Response(state);Check(Control<Button>("Contest").Visibility==Visibility.Collapsed,"Contest shown although the engine did not offer it.");
   state["contestAvailable"]=true;Response(state);Check(Control<Button>("Contest").Visibility==Visibility.Visible,"Contest action missing when every seat is taken.");
   state["contestAvailable"]=false;Response(state);Check(Control<Button>("Contest").Visibility==Visibility.Collapsed,"Contest stayed visible.");
   state["bondHeld"]="5";state["bondRemaining"]="15";state["reactivation"]=D("lastAt",120,"cooldownBlocks",200,"allowedFrom",320);Response(state);
   Check(Control<TextBlock>("Mined").Text.Contains("Bond held: 5 PCOL · still to accrue: 15 PCOL")&&Control<TextBlock>("Mined").Text.Contains("next one allowed from block 320"),"Probation bond or reactivation clock missing.");
   state.Remove("bondRemaining");state.Remove("reactivation");state.Remove("bondHeld");Response(state);Check(!Control<TextBlock>("Mined").Text.Contains("still to accrue"),"Probation line shown without probation.");
   Response(Snapshot(new object[]{Entry(Transfer,"contest","Included")}));Check(CardText(0,0)=="Mining registration · contest","Contest history row missing.");
   const BindingFlags Static=BindingFlags.Static|BindingFlags.NonPublic;
   Func<string,object[],object> call=(name,a)=>typeof(Wallet).GetMethod(name,Static).Invoke(null,a);
   // D-V3-21: the creation burn apart from the gas fee and the amount, and the signed maximum that bounds both.
   var quote=Json.Deserialize<Dictionary<string,object>>("{\"type\":\"quote\",\"kind\":\"transfer\",\"amount\":\"1\",\"creationBurn\":\"0.008\",\"gasFee\":\"0.0021\",\"feeBound\":\"0.0101\",\"suggestedFeeBound\":\"0.012\",\"feeBoundSet\":true,\"burnNote\":[\"first\",\"second\"]}");
   var burn=(List<string>)call("BurnLines",new object[]{quote});
   Check(burn.Count==6&&burn[0]=="Creation burn (quoted): 0.008 PCOL"&&burn[1]=="Gas fee (at most): 0.0021 PCOL"&&burn[2]=="Maximum fee you sign: 0.0101 PCOL · it bounds the gas and the burn together (your maximum)"&&burn[3]=="Suggested maximum fee: 0.012 PCOL"&&burn[5]=="second","Creation burn lines wrong: "+string.Join(" | ",burn));
   Check(((List<string>)call("BurnLines",new object[]{D("kind","transfer","amount","1")})).Count==0,"Burn lines shown on a network without a creation burn.");
   var send=(Dictionary<string,object>)call("SendCommand",new object[]{"0x01","1",""});Check(!send.ContainsKey("fee_bound")&&(string)send["method"]=="quote_send","An empty maximum fee was sent.");
   send=(Dictionary<string,object>)call("SendCommand",new object[]{"0x01","1","0.02"});Check((string)send["fee_bound"]=="0.02","The maximum fee was not sent.");
   // V3-10/V3-14: the contest review and the candidate list.
   var contest=Json.Deserialize<Dictionary<string,object>>("{\"kind\":\"contest\",\"victim\":\"0x02\",\"amount\":\"0\",\"bondTarget\":\"20\",\"probation\":true,\"contest\":{\"kind\":\"pool\"},\"steps\":[\"a\",\"b\"],\"note\":\"n\"}");
   var lines=(List<string>)call("ContestLines",new object[]{contest});
   Check(lines.Count==5&&lines[0]=="Probation-pool contest of 0x02"&&lines[1]=="1. a"&&lines[3]=="Bond now: 0 PCOL · filled from your rewards up to 20 PCOL"&&lines[4]=="n","Contest review lines wrong: "+string.Join(" | ",lines));
   var candidates=Json.Deserialize<Dictionary<string,object>>("{\"type\":\"contestCandidates\",\"seat\":[{\"address\":\"0x03\",\"count\":0,\"threshold\":\"25\"}],\"pool\":[]}");
   var seats=(List<string>)call("CandidateLines",new object[]{candidates,"seat"});Check(seats.Count==1&&seats[0]=="0x03 · 0 shares, threshold 25","Contest candidates wrong.");
   Check(((List<string>)call("CandidateLines",new object[]{candidates,"pool"})).Count==0,"Pool candidates invented.");
   // T34, network upgrades (plan §5): the dry run's change picks the text; only an activation waits for a coordinated release.
   var outcome=typeof(Wallet).GetMethod("UpgradeOutcome",BindingFlags.Static|BindingFlags.NonPublic);Check(outcome!=null,"Network upgrade texts are missing.");
   Check((string)outcome.Invoke(null,new object[]{"rebind"})=="Release update: consensus rules and schedule unchanged — saved; you can connect now","Rebind text missing.");
   Check((string)outcome.Invoke(null,new object[]{"none"})=="Already applied","Already-applied text missing.");
   Check(((string)outcome.Invoke(null,new object[]{"activation"})).Contains("coordinated release"),"An activation lost its coordinated-release text.");
   var lines=(List<string>)typeof(Wallet).GetMethod("UpgradeLines",BindingFlags.Static|BindingFlags.NonPublic).Invoke(null,new object[]{Json.Deserialize<Dictionary<string,object>>("{\"appended\":[[2,15000],[3,20000]]}")});
   Check(lines.Count==2&&lines[0]=="Full mining range at block 15000"&&lines[1]=="Consensus version 3 at block 20000","Appended fork entries are not listed.");
   // Peer diagnostics while no peer is connected (VA-SPEC §2.5).
   var lonely=Snapshot(new object[0]);lonely["peers"]=0;Response(lonely);Check(Control<TextBlock>("WalletNote").Text.StartsWith("Looking for peers"),"Plain peer search text missing.");
   lonely["legacyHellos"]=2;Response(lonely);Check(Control<TextBlock>("WalletNote").Text.StartsWith("Seeds still run PoCol 0.4.0"),"0.4.0 seed text missing.");
   lonely["incompatiblePeers"]=D("network",0,"wire",0,"consensus",1);lonely["lastIncompatibleReason"]="peer seed-1: peer runs consensus rules 2";Response(lonely);
   Check(Control<TextBlock>("WalletNote").Text.StartsWith("Peers use a different network configuration")&&Control<TextBlock>("WalletNote").Text.EndsWith("peer runs consensus rules 2"),"Consensus refusal reason not shown.");
   transfer["state"]="Included";transfer["confirmations"]=2;state["type"]="status";Response(state);
   if(args.Length==1){
    Invoke("Page","Activity");window.Show();window.UpdateLayout();
    Control<TextBlock>("Message").Text="UI regression fixture · sample transactions, not a live account";
    window.UpdateLayout();var content=(FrameworkElement)window.Content;
    var bitmap=new RenderTargetBitmap((int)Math.Ceiling(content.ActualWidth),(int)Math.Ceiling(content.ActualHeight),96,96,PixelFormats.Pbgra32);bitmap.Render(content);
    var png=new PngBitmapEncoder();png.Frames.Add(BitmapFrame.Create(bitmap));using(var file=File.Create(args[0]))png.Save(file);
   }
   window.Close();app.Shutdown();Check(Directory.Exists(home)==homeExisted,"UI test unexpectedly created a wallet home.");
   Console.WriteLine("PASS actual WPF history rendering, exact amounts, receipt transitions, direct-peer label, mining acknowledgement, refresh, rejoin, probation top-up, contest and creation-burn review texts, network upgrade texts and peer diagnostics");return 0;
  }catch(Exception e){Console.WriteLine("FAIL "+e.GetBaseException().Message);return 1;}
 }
}
