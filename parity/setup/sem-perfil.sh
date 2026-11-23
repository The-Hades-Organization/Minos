# Repositório cujo remote não tem perfil em packs/: sem projeto ativo, nada é dispensado da guarda de nome.
mkdir -p sem-perfil
git -C sem-perfil init -q
git -C sem-perfil remote add origin https://github.com/acme/sem-perfil.git
