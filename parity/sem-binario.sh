#!/usr/bin/env bash
# Guarda sem `cerberus` nem `hades` no PATH (plano F4 Minos §2.2, divergência D1): a guarda `hades cerberus-check
# porta/segredo-diff` da skill roda em processo no binário. Só Rust: o TS não roda sem o `hades`.
#   sem-binario.sh <skill montada> <binário minos>
# Prova: o token que só o Cerberus pega (a varredura em memória deixa passar) é barrado, nada é gravado, e a página
# limpa sai. Se o spawn fosse tentado, a guarda falharia por `hades` ausente e a página limpa também cairia.
set -euo pipefail
skill=$(cd "$1" && pwd)
rust=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
fix=$(cd "$(dirname "$0")/fixtures/guarda" && pwd)
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
falha() { echo "sem-binario: $*" >&2; exit 1; }

caminho=/usr/bin:/bin
for b in hades cerberus; do
  if PATH=$caminho command -v "$b" >/dev/null; then falha "$b está em $caminho; o caso perde o sentido"; fi
done
grep -q '"hades"' "$skill/loop/cerberus/porta/segredo-diff.json" || falha "a guarda da skill não é mais hades cerberus-check"

cp -a "$fix/." "$T/"
cd "$T"
roda() {
  env -i HOME="$T" HADES_DATA="$T/data" HADES_PACKS_ROOT="$T/packs" HADES_WORK_ROOT="$T/work" \
    HADES_SKILL_DIR="$skill" HADES_NOW=2026-11-20T12:00:00Z TZ=UTC LANG=C.UTF-8 NO_COLOR=1 PATH="$caminho" \
    "$rust" publish explicacao "$1" >"$T/saida.txt" 2>&1
}

rc=0; roda cerberus-google.json || rc=$?
[ "$rc" -eq 1 ] || { cat "$T/saida.txt" >&2; falha "token do Google: rc=$rc, esperado 1"; }
grep -q 'token do Google' "$T/saida.txt" || { cat "$T/saida.txt" >&2; falha "o veredito não veio do porta/segredo-diff"; }
grep -q 'AIzaSy' "$T/saida.txt" && falha "o valor do token apareceu na saída"
[ ! -e "$T/data/minos" ] || falha "a recusa gravou em data/minos"

rc=0; roda email-noreply.json || rc=$?
[ "$rc" -eq 0 ] || { cat "$T/saida.txt" >&2; falha "página limpa: rc=$rc, esperado 0"; }
[ -f "$T/data/minos/explicacao-email-noreply/index.html" ] || falha "página limpa sem index.html"
echo "sem-binario: ok (guarda em processo, sem hades nem cerberus no PATH)"
