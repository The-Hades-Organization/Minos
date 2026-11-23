#!/usr/bin/env bash
# Concorrência sobre o mesmo $D/minos/outputs.json (plano F4 Minos §2.1): 8 `link` em paralelo, cada um numa saída
# diferente; o registro final tem de ter os 8 links. Sem lock, o ler-mesclar-gravar perde gravações.
#   concorrencia.sh <skill montada> <binário minos> [rust|misto]
# rust: os 8 pelo binário (o flock de $D/minos/.lock). misto: 4 pelo TS e 4 pelo Rust; só passa quando o saveOutput do
# TS tomar o mesmo flock (PR da flag). No PR do publish Rust, trocar o link pelo publish (8 fontes diferentes).
set -euo pipefail
skill=$(cd "$1" && pwd)
rust=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
modo=${3:-rust}
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
export HOME=$T HADES_DATA=$T/data HADES_SKILL_DIR=$skill TZ=UTC LANG=C.UTF-8 NO_COLOR=1
mkdir -p "$T/data/minos"
python3 - "$T/data/minos/outputs.json" <<'PY'
import json, sys
rec = lambda i: {"model": "proposta", "scope": "personal", "plan": f"/srv/demo/p{i}.json", "url": None,
                 "file": f"/srv/demo/p{i}/index.html", "updatedAt": "2026-01-01T00:00:00.000Z"}
json.dump({"outputs": {f"proposta-p{i}": rec(i) for i in range(1, 9)}}, open(sys.argv[1], "w"), indent=2)
PY
pids=()
for i in 1 2 3 4 5 6 7 8; do
  (
    if [ "$modo" = misto ] && [ $((i % 2)) -eq 1 ]; then
      bun "$skill/src/cli.ts" minos link "proposta-p$i" "https://claude.ai/artifact/p$i" >/dev/null
    else
      "$rust" link "proposta-p$i" "https://claude.ai/artifact/p$i" >/dev/null
    fi
  ) &
  pids+=($!)
done
for p in "${pids[@]}"; do wait "$p"; done
python3 - "$T/data/minos/outputs.json" <<'PY'
import json, sys
outs = json.load(open(sys.argv[1]))["outputs"]
faltam = [k for k, v in outs.items() if v["url"] != f"https://claude.ai/artifact/{k.removeprefix('proposta-')}"]
if len(outs) != 8 or faltam:
    print(f"concorrência: {len(outs)} saídas, sem o link: {faltam}", file=sys.stderr)
    sys.exit(1)
print("concorrência: ok (8 links em paralelo, nenhum perdido)")
PY
