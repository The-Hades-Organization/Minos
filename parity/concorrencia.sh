#!/usr/bin/env bash
# Concorrência sobre o mesmo $D/minos/outputs.json (plano F4 Minos §2.1): 8 `publish` em paralelo, cada um de uma
# fonte diferente; o registro final tem de ter as 8 saídas. Sem lock, o ler-mesclar-gravar perde gravações.
#   concorrencia.sh <skill montada> <binário minos> [rust|misto]
# rust: os 8 pelo binário (o flock de $D/minos/.lock). misto: 4 pelo TS e 4 pelo Rust; só passa quando o saveOutput do
# TS tomar o mesmo flock (PR da flag).
set -euo pipefail
skill=$(cd "$1" && pwd)
rust=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
modo=${3:-rust}
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
export HOME=$T HADES_DATA=$T/data HADES_SKILL_DIR=$skill TZ=UTC LANG=C.UTF-8 NO_COLOR=1
mkdir -p "$T/data/minos"
for i in 1 2 3 4 5 6 7 8; do cp "$(dirname "$0")/fixtures/contrato/e-exemplo.json" "$T/p$i.json"; done
cd "$T"
pids=()
for i in 1 2 3 4 5 6 7 8; do
  (
    if [ "$modo" = misto ] && [ $((i % 2)) -eq 1 ]; then
      bun "$skill/src/cli.ts" minos publish explicacao "p$i.json" >/dev/null
    else
      "$rust" publish explicacao "p$i.json" >/dev/null
    fi
  ) &
  pids+=($!)
done
for p in "${pids[@]}"; do wait "$p"; done
python3 - "$T/data/minos/outputs.json" <<'PY'
import json, sys
outs = json.load(open(sys.argv[1]))["outputs"]
faltam = [f"explicacao-p{i}" for i in range(1, 9) if f"explicacao-p{i}" not in outs]
if len(outs) != 8 or faltam:
    print(f"concorrência: {len(outs)} saídas, faltam: {faltam}", file=sys.stderr)
    sys.exit(1)
print("concorrência: ok (8 publish em paralelo, nenhuma saída perdida)")
PY
