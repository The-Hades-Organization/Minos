# credenciais.md do caso: um valor que vale como segredo e um curto (< 8) que não conta (minos-guard.ts:12-21).
mkdir -p data
printf '%s\n' '# cofre' "- \`COFRE_CHAVE=cofre-$(printf 'x9Y8%.0s' 1 2 3 4)\`" 'CURTO=abc' > data/credenciais.md
