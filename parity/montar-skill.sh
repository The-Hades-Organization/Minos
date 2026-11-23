#!/usr/bin/env bash
# Monta a skill da matriz: cópia dos arquivos versionados do Hades-Skill (o lado TS roda dela e o Rust a lê por
# HADES_SKILL_DIR), com o parity/fixtures/skill-min por cima (config, modelo da página e a guarda da Porta) e os stubs
# de parity/stubs no PATH dos casos (loop/test/stubs).
#   montar-skill.sh <Hades-Skill> <destino>
set -euo pipefail
src=$(cd "$1" && pwd); dest=$2
here=$(cd "$(dirname "$0")" && pwd)
rm -rf "$dest"; mkdir -p "$dest"
(cd "$src" && git ls-files -z | tar -cf - --null -T -) | tar -xf - -C "$dest"
rm -rf "$dest/loop/cerberus" "$dest/kit/config.tsv"
cp -a "$here/fixtures/skill-min/." "$dest/"
cp -a "$here/stubs/." "$dest/loop/test/stubs/"
[ -d "$src/node_modules" ] && ln -s "$src/node_modules" "$dest/node_modules"
echo "$dest"
