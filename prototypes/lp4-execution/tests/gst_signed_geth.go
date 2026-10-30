// Independent test-only signed Cancun admission using unmodified Geth1.14.12.
// Not linked into PoCol, no keys, network, boot, or binary redistribution.
package main
import (
 "bufio"
 "encoding/hex"
 "errors"
 "encoding/json"
 "fmt"
 "math/big"
 "os"
 "strings"
 "github.com/ethereum/go-ethereum/common"
 "github.com/ethereum/go-ethereum/consensus/misc/eip4844"
 "github.com/ethereum/go-ethereum/core"
 "github.com/ethereum/go-ethereum/core/state"
 "github.com/ethereum/go-ethereum/core/tracing"
 "github.com/ethereum/go-ethereum/core/types"
 "github.com/ethereum/go-ethereum/core/vm"
 "github.com/ethereum/go-ethereum/params"
 "github.com/holiman/uint256"
)
type account struct {Balance string `json:"balance"`;Nonce string `json:"nonce"`;Code string `json:"code"`;Storage map[string]string `json:"storage"`}
type input struct {Raw string `json:"raw"`;Pre map[string]account `json:"pre"`;Env map[string]string `json:"env"`}
func num(s string)*big.Int {if s=="" {return new(big.Int)};v,ok:=new(big.Int).SetString(strings.TrimPrefix(s,"0x"),16);if !ok {panic("invalid public fixture integer")};return v}
func raw(s string)[]byte{b,e:=hex.DecodeString(strings.TrimPrefix(s,"0x"));if e!=nil {panic(e)};return b}
func result(stage string,err error)map[string]interface{}{
 kind:="other"
 checks:=[]struct{err error;name string}{{core.ErrIntrinsicGas,"intrinsic-gas"},{core.ErrInsufficientFunds,"insufficient-funds"},{core.ErrInsufficientFundsForTransfer,"insufficient-funds"},{core.ErrNonceMax,"nonce-max"},{core.ErrGasLimitReached,"block-gas"},{core.ErrTipAboveFeeCap,"priority-fee"},{core.ErrFeeCapTooLow,"fee-cap"},{core.ErrMaxInitCodeSizeExceeded,"initcode-size"},{core.ErrSenderNoEOA,"sender-not-eoa"},{core.ErrBlobFeeCapTooLow,"blob-fee-cap"},{core.ErrMissingBlobHashes,"missing-blobs"},{core.ErrBlobTxCreate,"blob-create"}}
 for _,check:=range checks{if errors.Is(err,check.err){kind=check.name;break}}
 return map[string]interface{}{"stage":stage,"accepted":false,"error":err.Error(),"errorKind":kind}
}
func run(in input)map[string]interface{}{
 var tx types.Transaction
 if err:=tx.UnmarshalBinary(raw(in.Raw));err!=nil{return result("wire",err)}
 cfg:=*params.AllDevChainProtocolChanges;cfg.ChainID=big.NewInt(777902);cfg.PragueTime=nil
 excess:=num(in.Env["currentExcessBlobGas"]).Uint64()
 h:=&types.Header{Number:num(in.Env["currentNumber"]),Time:num(in.Env["currentTimestamp"]).Uint64(),GasLimit:num(in.Env["currentGasLimit"]).Uint64(),BaseFee:num(in.Env["currentBaseFee"]),Difficulty:new(big.Int),Coinbase:common.HexToAddress(in.Env["currentCoinbase"]),MixDigest:common.HexToHash(in.Env["currentRandom"]),ExcessBlobGas:&excess}
 // State transition alone does not enforce the per-block blob allowance.
 // Use Geth's real header validator, with a synthetic parent whose target blob
 // usage preserves the fixture's current excess. This is a header-layer witness,
 // not a recovered original parent or a full-block acceptance claim.
 blobGas:=tx.BlobGas();h.BlobGasUsed=&blobGas
 parentUsage:=uint64(params.BlobTxTargetBlobGasPerBlock)
 parent:=&types.Header{ExcessBlobGas:&excess,BlobGasUsed:&parentUsage}
 if err:=eip4844.VerifyEIP4844Header(parent,h);err!=nil{return result("headerBlobAllowance",err)}
 msg,err:=core.TransactionToMessage(&tx,types.MakeSigner(&cfg,h.Number,h.Time),h.BaseFee);if err!=nil{return result("signature",err)}
 st,err:=state.New(types.EmptyRootHash,state.NewDatabaseForTesting());if err!=nil{panic(err)}
 for address,a:=range in.Pre {addr:=common.HexToAddress(address);balance,overflow:=uint256.FromBig(num(a.Balance));if overflow {panic("invalid fixture allocation")};st.SetBalance(addr,balance,tracing.BalanceIncreaseGenesisBalance);st.SetNonce(addr,num(a.Nonce).Uint64());st.SetCode(addr,raw(a.Code));for k,v:=range a.Storage{st.SetState(addr,common.HexToHash(k),common.HexToHash(v))}}
 st.Finalise(true);before:=st.IntermediateRoot(true);snapshot:=st.Snapshot()
 context:=core.NewEVMBlockContext(h,nil,&h.Coinbase);context.GetHash=func(uint64)common.Hash{return common.Hash{}}
 evm:=vm.NewEVM(context,core.NewEVMTxContext(msg),st,&cfg,vm.Config{})
 _,err=core.ApplyMessage(evm,msg,new(core.GasPool).AddGas(h.GasLimit))
 if err!=nil{st.RevertToSnapshot(snapshot);if st.IntermediateRoot(true)!=before {panic("independent rejection rollback changed root")};r:=result("execution",err);r["sender"]=strings.ToLower(msg.From.Hex());r["hash"]=strings.ToLower(tx.Hash().Hex());r["snapshotRestored"]=true;return r}
 return map[string]interface{}{"accepted":true,"stage":"execution"}
}
func main(){scanner:=bufio.NewScanner(os.Stdin);scanner.Buffer(make([]byte,65536),16*1024*1024);writer:=bufio.NewWriter(os.Stdout);defer writer.Flush();for scanner.Scan(){var in input;if err:=json.Unmarshal(scanner.Bytes(),&in);err!=nil{panic(err)};out:=run(in);b,err:=json.Marshal(out);if err!=nil{panic(err)};fmt.Fprintln(writer,string(b));writer.Flush()};if err:=scanner.Err();err!=nil{panic(err)}}
