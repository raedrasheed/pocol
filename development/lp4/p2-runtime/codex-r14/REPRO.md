# Reproduction and exact scope

Use Python3.12 and historical python-requirements.lock. For the new native-compression
profile only, install --only-binary=:all: --no-deps --require-hashes from
python-comparison-extra.lock (pinned Linux x86_64 CPython3.12 wheel). Retain160-vector
backend differential; do not retroactively rerun/relabel historical evidence with extras.

```
python prototypes/lp4-execution/tests/gst_admission_checks.py
python prototypes/lp4-execution/tests/gst_admission_mutations.py OUTPUT
python prototypes/lp4-execution/tests/gst_blake2_checks.py
python prototypes/lp4-execution/tests/gst_independent.py CORPUS OUTPUT --profile lp4-create01-modexp-admission --timeout 300
```

Native reference: official Go1.23.2; Geth v1.14.12. Normal CI uses pinned setup-go,
go install github.com/ethereum/go-ethereum/cmd/evm@v1.14.12 and default checksum DB.
Local restricted source access required the explicit same-version metadata replacements
in geth-mirror-build-replacements.json. Copy the ORIGINAL immutable module source to
an isolated source directory and apply those VERSION-SPECIFIC Go module replacements;
retain original go.mod/go.sum for canonical verification. Never change EVM Go files.
Record actual target closure with go list -deps -json -mod=readonly -buildvcs=false;
convert its Module objects to the closure JSON. Run:

```
python prototypes/lp4-execution/tests/geth_mirror_verify.py ORIGINAL_GETH_MODULE GOMODCACHE MIRRORS.json PROOF.json CLOSURE.json
```

All used source ZIPs must reproduce original canonical go.sum h1, including file names.
Verify embedded binary go version -m module pairs equal actual target closure. Verify
all non-go.mod/go.sum source files byte-identical. Build offline with GOPROXY=off,
go build -mod=readonly -buildvcs=false -p2 -o EVM ./cmd/evm. Isolated GOPATH/GOCACHE,
GOMAXPROCS2; compiler/source/build flags/binary digests retained. No network-policy
change, proxy removal or concealed dependency substitution is allowed.

```
python prototypes/lp4-execution/tests/gst_geth_control.py CORPUS EVM OUTPUT
python prototypes/lp4-execution/tests/gst_geth_mutations.py OUTPUT GETH_SOURCE GO_BINARY CORPUS EVM
```

Require20661 unique cases,2807 input files, no limit flag, exit0, no failures. Native
case per-name ordinals follow the pinned StateTest.Subtests ascending Index loop;
collector preserves that order. All counts and exact identities checked against R4.
100 input files have no Cancun variant: null is allowed ONLY for those zero cases.
Unexpected/truncated output, missing/extra variants, parser errors or engine errors
must fail, not disappear. Process receipt is written BEFORE interpretation.
No -dump/-debug/trace flags, fixture keys or arbitrary native errors are published.
Go controls build separate source overlays and must fail actual root assertions;
compile failure never counts. The corpus is public fixture input, never live keys.
