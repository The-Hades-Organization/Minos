# JSON de dados com 2 MiB + 1 byte: o publish recusa antes de ler (LP:43).
printf '{"x":"' > grande.json
head -c 2097145 /dev/zero | tr '\0' a >> grande.json
printf '"}' >> grande.json
