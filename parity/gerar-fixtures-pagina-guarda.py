#!/usr/bin/env python3
"""Gera parity/fixtures/{pagina,guarda} a partir dos exemplos de parity/fixtures/contrato. Dados fictícios.

Rodar da raiz do repo: python3 parity/gerar-fixtures-pagina-guarda.py
"""
import copy
import json
import os
import shutil

HERE = os.path.dirname(os.path.abspath(__file__))
FIX = os.path.join(HERE, "fixtures")
E = json.load(open(os.path.join(FIX, "contrato", "e-exemplo.json")))
P = json.load(open(os.path.join(FIX, "contrato", "p-exemplo.json")))

# montado em partes: o arquivo não casa com a varredura de segredos
TOKEN_X = "gh" + "p_" + "x" * 36  # casa o padrão, mas o Cerberus o lê como exemplo (xxxx)
TOKEN = "gh" + "p_" + "a1B2" * 9
JWT = "eyJh" + "bGciOiJIUzI1NiJ9.e30.x"
GOOGLE = "AIza" + "SyAbCdEfGhIjKlMnOpQrStUvWxYz0123456"  # só o Cerberus (porta/segredo-diff) pega
ATRIB = 'api_key: "' + "AbCdEfGh12345678ZZ" + '"'  # na página as aspas saem escapadas
VAR = "INTERNAL_SERVICE_" + "TOKEN=" + "abcd1234efgh"
EMAIL = "fulano@acme-demo.dev"
CAMINHO = "/home/fulano/projeto/x"
COFRE = "cofre-" + "x9Y8" * 4


def expl(**over):
    d = copy.deepcopy(E)
    d.update(over)
    return d


def prop(**over):
    d = copy.deepcopy(P)
    d.update(over)
    return d


def secao(paragrafos=None, snippet=None):
    code = [] if snippet is None else [{"file": "a.ts", "snippet": snippet}]
    return [{"title": "s", "paragraphs": paragrafos or [], "code": code}]


def grava(pasta, nome, doc):
    path = os.path.join(pasta, nome)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        if isinstance(doc, str):
            f.write(doc)
        else:
            json.dump(doc, f, ensure_ascii=False, indent=2)
            f.write("\n")


def pagina():
    d = os.path.join(FIX, "pagina")
    shutil.rmtree(d, ignore_errors=True)
    grava(d, "hostil-titulo.json", expl(title="<script>alert(1)</script>"))
    grava(d, "hostil-crase.json", expl(sections=secao(["`<img src=x>`"])))
    grava(d, "hostil-trecho.json", expl(sections=secao([], "</pre><script>x()</script>")))
    grava(d, "crase-mista.json", expl(sections=secao(["usa `a<b>` e <i>"])))
    grava(d, "crase-impar.json", expl(sections=secao(["abre `a e nunca fecha <b>"])))
    grava(d, "crase-varias.json", expl(sections=secao(["`a` e `b&c` e `` vazio, \"aspas\" & 'apóstrofo'"])))
    grava(d, "hostil-campos.json", expl(
        question="<b>q</b> & `<u>`",
        summary="resumo com <a href=\"javascript:x\">link</a>",
        source={"repo": "dono/<repo>", "ref": "main@<abc>", "paths": ["src/<x>/"]},
        flow=[{"step": "`<s>` passo", "where": "a.ts:<1>", "detail": "<em>d</em>"}],
        files=[{"path": "<p>.ts", "role": "papel `<r>`"}],
        glossary=[{"term": "<t>", "meaning": "`<m>` & m"}],
        caveats=["<c> `x`"], nextQuestions=["<n>?"],
    ))
    hostil_p = prop(title="<script>p</script>", summary="usa `<x>` & <y>")
    hostil_p["options"][0]["title"] = "<o>"
    hostil_p["options"][0]["pros"] = ["`<pro>`"]
    hostil_p["risks"] = [{"risk": "<r>", "mitigation": "`<m>`"}] if isinstance(P.get("risks"), list) and P["risks"] and isinstance(P["risks"][0], dict) else ["<r>"]
    grava(d, "hostil-proposta.json", hostil_p)
    for kind in ["implementacao", "correcao", "fluxo", "refatoracao", "discussao"]:
        grava(d, f"tipo-{kind}.json", prop(kind=kind))
    grava(d, "Retry Pedidos.json", prop())
    grava(d, "fluxo.json", expl())
    grava(d, "Ação_Nova.JSON", expl())
    grava(d, "@@@.json", expl())
    grava(d, "x.md.json", expl())
    grava(d, ("nome-bem-comprido-" * 5) + ".json", expl())


def perfil(nome, repo, vocab, aliases=None):
    doc = {"schema": "hades.profile/1", "name": nome, "identity": {"repo": repo, "vocabulary": vocab}}
    if aliases:
        doc["identity"]["aliases"] = aliases
    return doc


def guarda():
    d = os.path.join(FIX, "guarda")
    shutil.rmtree(d, ignore_errors=True)
    grava(d, "packs/acme-demo/profile.json", perfil("acme-demo", "acme/demo", ["Pedidoteca"]))
    grava(d, "packs/outro-demo/profile.json", perfil("outro-demo", "outra/outro-demo", ["Faturix"], ["outrinho"]))
    ativo = {"repo": "acme/demo", "ref": "main@abc1234", "paths": ["src/pedidos/"]}
    grava(d, "ativo-e.json", expl(source=ativo))
    grava(d, "ativo-p.json", prop(source={"repo": "acme/demo", "ref": None, "paths": []}))
    grava(d, "ativo-nome.json", expl(summary="Como o acme-demo grava pedidos."))
    grava(d, "ativo-vocab.json", expl(summary="A Pedidoteca guarda o pedido."))
    grava(d, "outro-repo.json", expl(source={"repo": "outra/outro-demo", "ref": None, "paths": []}))
    grava(d, "outro-nome.json", expl(summary="Igual ao outro-demo."))
    grava(d, "outro-alias.json", expl(summary="Igual ao outrinho."))
    grava(d, "outro-vocab.json", expl(summary="Igual ao Faturix."))
    grava(d, "ativo-email.json", expl(source=ativo, caveats=[f"fale com {EMAIL}"]))
    grava(d, "cred-trecho.json", expl(sections=secao([], f'const t = "{TOKEN}";')))
    grava(d, "cred-proposta.json", prop(context=[f"token {TOKEN}"]))
    grava(d, "cred-exemplo.json", expl(sections=secao([], f'const t = "{TOKEN_X}";')))
    grava(d, "cred-jwt.json", expl(caveats=[f"jwt {JWT}"]))
    grava(d, "cred-var.json", expl(caveats=[VAR]))
    grava(d, "cerberus-google.json", expl(caveats=[f"chave {GOOGLE}"]))
    grava(d, "cerberus-atribuicao.json", expl(caveats=[ATRIB]))
    grava(d, "caminho.json", expl(caveats=[f"veja {CAMINHO}"]))
    grava(d, "email.json", expl(caveats=[f"fale com {EMAIL}"]))
    grava(d, "email-noreply.json", expl(caveats=["fale com noreply@example.com"]))
    grava(d, "cofre.json", expl(caveats=[f"usando {COFRE} aqui"]))
    grava(d, "cofre-curto.json", expl(caveats=["usando abc aqui"]))
    grava(d, "varios.json", expl(
        caveats=[f"veja {CAMINHO}", f"fale com {EMAIL}"],
        sections=secao([f"token {TOKEN}"]),
    ))
    grava(d, "planos/cred.md", f"# Plano — Guarda\n\n## Objetivo\n\nToken de teste: {TOKEN}\n")
    grava(d, "planos/ativo.md", "# Plano — Guarda\n\n## Objetivo\n\nEntrega do acme-demo.\n")
    grava(d, "data/.keep", "")


if __name__ == "__main__":
    pagina()
    guarda()
