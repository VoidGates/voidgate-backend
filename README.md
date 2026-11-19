# O código está no repositório único

O VoidGate agora está em [VoidGates/VoidGate](https://github.com/VoidGates/VoidGate).

A página é a mesma de antes. Os dois endereços abrem o mesmo site: https://voidgates.github.io/VoidGate/ e https://voidgates.github.io/voidgate-frontend/.

O texto abaixo ficou de quando a API e o site eram repositórios separados.

---
# VoidGate backend

O VoidGate é uma camada adicional e opcional de segurança para aplicações e fluxos de IA. Este repositório contém um esqueleto de API em Rust. Nesta etapa, ele **decide** se um caminho HTTP configurado deve ser encaminhado para uma isca e se uma mensagem de chat pode seguir; o gateway que integra a API precisa aplicar a decisão. O serviço ainda não recebe tráfego como proxy nem executa um ambiente-isca.

## Requisitos

- Rust estável com Cargo.
- Uma chave de serviço aleatória de 32 a 128 bytes, fornecida em `VOIDGATE_API_KEY`. Ela não pode ter espaços nas pontas nem caracteres de controle.

As versões das dependências estão fixadas em `Cargo.lock`. Use `--locked` para não atualizar esse arquivo sem perceber.

## Configuração e execução

Em desenvolvimento, copie `.env.example` para `.env`, preencha a chave e rode `cargo run --locked`. O programa lê o `.env` da pasta em que é executado; o arquivo está no `.gitignore` e nunca deve ser versionado. Só linhas `NOME=valor` simples são aceitas: sem aspas, sem `export` e sem comentário na mesma linha. Salve o arquivo em UTF-8.

Uma variável definida no ambiente do processo sempre vence o `.env`. Em produção não há `.env`: configure pelo ambiente.

| Variável | Obrigatória | Uso |
| --- | --- | --- |
| `VOIDGATE_API_KEY` | Sim | Chave enviada no cabeçalho `x-voidgate-key` da rota de decisão. |
| `VOIDGATE_DECOY_PATHS` | Não | Caminhos exatos separados por vírgula; padrão: nenhum. |
| `VOIDGATE_BIND` | Não | Endereço de escuta; padrão: `127.0.0.1:8080`. |
| `VOIDGATE_RATE_LIMIT_PER_MINUTE` | Não | Pedidos de decisão por minuto para a credencial; de 1 a 10000, padrão: `60`. |
| `VOIDGATE_DEV_DOCS` | Não | `1` liga a tela de testes em `/docs`; `0` ou ausente desliga. Só existe em build de desenvolvimento. |

Sem `.env`, exemplo para gerar uma chave no PowerShell e rodar só com variáveis de ambiente:

```powershell
$rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$rng.GetBytes($bytes)
$env:VOIDGATE_API_KEY = [Convert]::ToBase64String($bytes)
$rng.Dispose()
$env:VOIDGATE_DECOY_PATHS = '/internal/backup'
cargo run --locked
```

O processo se recusa a iniciar, com uma mensagem `error: ...`, quando a configuração está errada. Ele nunca sobe com uma defesa desligada por engano:

- chave ausente, fora do tamanho ou impossível de enviar em um cabeçalho;
- caminho de isca inválido, `VOIDGATE_BIND` inválido ou porta em uso;
- `.env` ilegível (por exemplo, salvo em UTF-16 pelo PowerShell);
- nome desconhecido com prefixo `VOIDGATE_`, no `.env` ou no ambiente, como `VOIDGATE_DECOY_PATH` sem o S;
- `VOIDGATE_DEV_DOCS` diferente de `0` e `1`, ou `VOIDGATE_RATE_LIMIT_PER_MINUTE` que não seja um número de 1 a 10000.

Ao subir, o processo imprime `voidgate-backend listening on http://<endereço>`. Os valores opcionais ausentes usam os padrões acima.

### Encerramento

Ao receber Ctrl+C, ou SIGTERM em sistemas Unix, o processo para de aceitar conexões, deixa cada conexão aberta terminar a requisição em andamento e só então encerra. Conexões ociosas são fechadas na hora. Se alguma requisição não terminar em 10 s, ela é interrompida.

### Limites de conexão

| Limite | Valor |
| --- | --- |
| Conexões simultâneas | 1024; as seguintes aguardam na fila do sistema operacional |
| Tempo para enviar os cabeçalhos, e tempo máximo de conexão ociosa | 30 s; depois a conexão é encerrada |
| Tempo de uma decisão, contando a chegada do corpo | 5 s; depois a resposta é `504` |
| Decisões simultâneas | 256; acima disso a resposta é `503` |

Os valores de conexão ficam em `src/server.rs`; os de decisão, em `src/runtime.rs` e `src/state.rs`. Eles limitam o consumo de recursos, mas não impedem que um único cliente ocupe todas as conexões: o limite por origem é papel do gateway.

## Tela de testes (só em desenvolvimento)

Com `VOIDGATE_DEV_DOCS=1`, o serviço publica o Swagger UI em `http://127.0.0.1:8080/docs` e a especificação OpenAPI em `/docs/openapi.json`:

```powershell
$env:VOIDGATE_DEV_DOCS = '1'
cargo run --locked
```

Abra `/docs`, clique em **Authorize**, cole o valor de `VOIDGATE_API_KEY` e use **Try it out** em cada rota. O arquivo `/docs/openapi.json` também pode ser importado no Postman, Insomnia ou Bruno.

A tela descreve a API inteira, então ela não existe fora do desenvolvimento:

- O binário de release (`cargo build --release`) não contém a página nem a especificação, e se recusa a iniciar com `VOIDGATE_DEV_DOCS=1`.
- Sem a variável, `/docs` responde como qualquer rota desconhecida.
- Com a variável e um `VOIDGATE_BIND` que não seja loopback, o processo se recusa a iniciar.
- A tela só responde a pedidos cujo cabeçalho `Host` seja `127.0.0.1`, `localhost` ou `[::1]`. Um pedido repassado por um proxy, ou feito por uma página externa, recebe a mesma resposta de uma rota inexistente.
- A página carrega o Swagger UI 5.17.14 do cdnjs com hash de integridade e uma política CSP restrita; o navegador precisa de internet na primeira abertura.

A especificação é escrita à mão em `src/docs/openapi.json`. Ao criar ou alterar uma rota, atualize esse arquivo. Um teste falha se uma das rotas atuais sair dele, ou se os códigos de erro da especificação e do código deixarem de ser os mesmos; uma rota nova sem documentação não é detectada.

## Erros

Todo erro sai no mesmo formato, com código e mensagem fixos:

```json
{ "error": { "code": "invalid_body", "message": "Request body does not match the documented contract." } }
```

| Status | `code` | Quando |
| --- | --- | --- |
| 401 | `unauthorized` | Chave ausente ou inválida. |
| 400 | `invalid_json` | O corpo não é JSON válido. |
| 400 | `invalid_body` | Campo ausente, extra ou com valor fora do contrato. |
| 413 | `payload_too_large` | Corpo ou `content` acima do limite. |
| 415 | `unsupported_media_type` | Sem `Content-Type: application/json`. |
| 429 | `rate_limited` | Limite de pedidos por minuto excedido; traz o cabeçalho `Retry-After`. |
| 503 | `unavailable` | 256 decisões já em andamento, ou limitador indisponível. |
| 504 | `decision_timeout` | A decisão passou de 5 s, contando a chegada do corpo. |
| 404 | `not_found` | Rota desconhecida (só com chave válida). |
| 405 | `method_not_allowed` | Método não aceito pela rota. |

O erro informa a categoria do problema, nunca a regra que falhou: não cita o campo, não repete o que foi enviado e não diz, por exemplo, qual caractere do caminho foi recusado. O contrato completo fica na documentação, que só quem desenvolve acessa.

Sem chave válida, toda rota fora as de saúde devolve o mesmo `401`, exista ela ou não, com qualquer método ou corpo. Quem não tem a chave não consegue mapear as rotas pelo tipo de erro.

As respostas desta API são para o gateway. Ele não deve repassar ao cliente final `reason_code`, `reason_codes`, `findings` nem o corpo de um erro: esses campos dizem qual defesa disparou. Para o cliente final, responda de forma genérica.

O módulo fica em `src/error.rs` (`AppError` e o extrator `ApiJson`); handlers e services novos devolvem `AppError` em vez de um status solto.

## Arquitetura

O projeto segue a **Arquitetura em Camadas** (em inglês, *Layered Architecture*). O código é dividido em camadas, cada uma com uma única responsabilidade, e uma camada só chama a que está logo abaixo dela:

```text
requisição → auth → handlers → services → domain
```

| Camada | Pasta | Responsabilidade | Não faz |
| --- | --- | --- | --- |
| Apresentação | `handlers/` | Recebe a requisição HTTP, chama o service e devolve JSON. | Não contém regra de negócio. |
| Aplicação | `services/` | Valida a entrada, consulta o domínio e monta a resposta. | Não conhece HTTP nem axum. |
| Domínio | `domain/` | Decide: caminho de isca e detectores de conteúdo. | Não conhece JSON, rotas nem configuração. |

Três partes são usadas por todas as camadas:

- `models/`: os corpos de requisição e resposta (DTOs), sem lógica.
- `error.rs`: o `AppError`, único formato de erro. Qualquer camada devolve um `AppError` e ele vira a resposta JSON em um só lugar.
- `state.rs`: o `AppState`, com a configuração lida na inicialização.

Em um sistema comum, a última camada seria a de dados (banco). Este serviço não tem banco: o estado é só leitura e vive em memória, então a camada mais interna é o domínio.

Na prática, isso define onde cada mudança entra:

- Rota nova: um handler em `handlers/`, um service em `services/`, os corpos em `models/` e a rota em `lib.rs`.
- Regra de decisão nova, como um detector: só em `domain/`.
- Erro novo: uma variante em `AppError`.

### Estrutura de pastas

```text
src/
├── main.rs        lê a configuração (.env e ambiente) e sobe o servidor
├── lib.rs         monta o roteador: rotas, autenticação, limites de corpo
├── server.rs      aceita conexões com timeouts, limite de conexões e encerramento gracioso
├── rate_limit.rs  limite de pedidos por credencial
├── runtime.rs     limite de decisões simultâneas e tempo máximo por decisão
├── metrics.rs     contadores agregados expostos em /metrics
├── state.rs       AppState: configuração compartilhada, criada na inicialização
├── auth.rs        middleware da chave x-voidgate-key
├── error.rs       AppError: o único formato de erro da API
├── handlers/      camada HTTP: extrai a requisição, chama o service, devolve JSON
├── services/      regras de negócio: valida a entrada e monta a resposta
├── models/        corpos de requisição e resposta (DTOs)
├── domain/        regras puras de decisão, sem HTTP
│   ├── policy.rs      caminhos de isca
│   └── detectors/     detectores de conteúdo
└── docs/          página do Swagger UI e openapi.json
```

## API atual

- `GET /health` e `GET /live` indicam que o processo está ativo. `GET /ready` verifica se o estado em memória do limitador está disponível. Não há dependências externas configuradas nesta etapa.
- `POST /v1/decisions/http` exige `x-voidgate-key` e JSON com `method` e `path`. O corpo é limitado a 4 KiB.

Exemplo no PowerShell, em outro terminal que também tenha `VOIDGATE_API_KEY` definida:

```powershell
Invoke-RestMethod -Uri 'http://127.0.0.1:8080/v1/decisions/http' -Method Post -Headers @{ 'x-voidgate-key' = $env:VOIDGATE_API_KEY } -ContentType 'application/json' -Body '{"method":"GET","path":"/internal/backup"}'
```

Para um caminho configurado, a resposta é `{"action":"decoy","reason_code":"registered_decoy_path"}`. Para outro caminho válido, é `{"action":"allow","reason_code":"no_matching_rule"}`. Caminhos ambíguos ou codificados são rejeitados; a comparação dos caminhos de isca é exata.

As rotas `/v1/decisions/*` aplicam um limite por credencial autenticada, depois da autenticação e antes de ler o JSON. O serviço aceita uma única chave nesta versão, e o limitador mantém buckets em memória com capacidade limitada. O algoritmo permite uma rajada de até `VOIDGATE_RATE_LIMIT_PER_MINUTE` pedidos e repõe a capacidade gradualmente ao longo de um minuto. Ao exceder o limite, a API responde `429 Too Many Requests` com `Retry-After` em segundos. Requisições sem chave válida respondem `401` e não consomem essa capacidade. Configure no gateway o limite por IP para tentativas sem chave; esta aplicação não confia em `X-Forwarded-For`. As rotas de saúde permanecem acessíveis. Cada réplica mantém seus próprios buckets, portanto o limite ainda não é global. O serviço também impõe no máximo 256 decisões concorrentes e encerra decisões que excedam 5 segundos com `504`. `GET /metrics` exige a chave da API e expõe apenas contadores agregados, sem conteúdo, caminhos ou credenciais.

### `POST /v1/decisions/chat`

Decide se uma mensagem de chat pode seguir. Exige `x-voidgate-key` e JSON com exatamente dois campos:

| Campo | Valores |
| --- | --- |
| `role` | `"user"`, `"system"` ou `"assistant"` |
| `content` | texto UTF-8 válido de até 16 KiB |

O corpo desta rota é limitado a 100 KiB, para que um `content` de 16 KiB caiba mesmo quando o JSON escapa cada caractere como `\uXXXX`. O `content` é inspecionado em memória e descartado: ele **nunca** aparece na resposta nem em logs; a resposta só traz posições em bytes do texto original.

Resposta: `{"action":"allow"|"block","reason_codes":[...],"findings":[{"code","detector","start","end"}],"truncated":false}`. `start` e `end` são offsets em bytes (fim exclusivo) do `content` enviado. Sem ocorrências, `action` é `allow` e as duas listas vêm vazias.

Caracteres ocultos seguidos formam um único item em `findings`, então uma frase escondida é um intervalo só. A lista traz no máximo 100 itens; `truncated` vem `true` quando havia mais. A decisão considera todas as ocorrências, mesmo as não listadas.

Códigos de motivo (`reason_codes`) nesta versão:

| Código | Detector | Dispara quando |
| --- | --- | --- |
| `content.invisible_unicode` | `invisible_unicode` | O texto contém um caractere que a pessoa não vê, mas o modelo lê. Qualquer ocorrência fora das exceções abaixo gera `block`. |

Caracteres tratados como ocultos:

| Grupo | Pontos de código |
| --- | --- |
| Largura zero e marcas de direção | U+200B–U+200F, U+2060–U+206F, U+FEFF |
| Controles bidirecionais (Trojan Source) | U+202A–U+202E, U+2066–U+2069, U+061C |
| Separadores de linha e parágrafo | U+2028, U+2029 |
| Caracteres tag e seletores suplementares (texto ASCII escondido) | U+E0000–U+E0FFF |
| Preenchimentos em branco | U+00AD, U+034F, U+115F, U+1160, U+17B4, U+17B5, U+180B–U+180F, U+3164, U+FFA0 |
| Controles de anotação e de formato musical | U+FFF9–U+FFFB, U+1D173–U+1D17A |
| Caracteres de controle | U+0000–U+001F (exceto tab, LF e CR) e U+007F–U+009F |

Exceções, porque fazem parte da escrita normal. Todas exigem vizinhos visíveis do tipo certo, então nenhuma serve para esconder texto:

| Caractere | Liberado quando |
| --- | --- |
| U+200D (ZWJ) | está entre dois emoji, como em emoji de profissão, família e bandeiras, ou entre duas letras árabes ou índicas |
| U+200C (ZWNJ) | está entre duas letras árabes ou índicas, como na ortografia persa |
| U+200E e U+200F (marcas de direção) | estão ao lado de texto hebraico ou árabe |
| U+FEFF (BOM) | é o primeiro caractere do texto |

Os seletores de variação U+FE00–U+FE0F não são bloqueados: U+FE0F faz parte de emoji comuns. Emoji e escritas são reconhecidos por faixas de código, não pelas tabelas completas do Unicode; se um texto legítimo for bloqueado, a faixa correspondente precisa entrar em `src/domain/detectors/invisible_unicode.rs`.

Respostas de erro, no formato da seção [Erros](#erros): `401` sem chave válida (antes de ler o corpo), `400` para JSON inválido, `role` desconhecida, campo extra ou faltante, `413` para `content` acima de 16 KiB ou corpo acima de 100 KiB, `415` sem `Content-Type: application/json`.

Exemplo no PowerShell. O corpo é enviado como bytes UTF-8 porque o Windows PowerShell 5.1 codifica strings com acento em ANSI, que o serviço rejeita com `400`:

```powershell
$body = [System.Text.Encoding]::UTF8.GetBytes('{"role":"user","content":"Olá, tudo bem?"}')
Invoke-RestMethod -Uri 'http://127.0.0.1:8080/v1/decisions/chat' -Method Post -Headers @{ 'x-voidgate-key' = $env:VOIDGATE_API_KEY } -ContentType 'application/json' -Body $body
```

Resposta: `{"action":"allow","reason_codes":[],"findings":[],"truncated":false}`. Com um U+202E (override da direita para a esquerda) após `Olá `:

```powershell
$body = [System.Text.Encoding]::UTF8.GetBytes('{"role":"user","content":"Olá ' + [char]0x202E + 'mundo"}')
Invoke-RestMethod -Uri 'http://127.0.0.1:8080/v1/decisions/chat' -Method Post -Headers @{ 'x-voidgate-key' = $env:VOIDGATE_API_KEY } -ContentType 'application/json' -Body $body | ConvertTo-Json -Depth 5 -Compress
```

Resposta: `{"action":"block","reason_codes":["content.invisible_unicode"],"findings":[{"code":"content.invisible_unicode","detector":"invisible_unicode","start":5,"end":8}],"truncated":false}` (`Olá ` ocupa 5 bytes e o U+202E ocupa 3).

Os detectores ficam em `src/domain/detectors/`, cada um implementando o trait `Detector`; novos detectores entram na lista `default_detectors()` sem alterar o handler nem o service.

## Verificação

```text
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

## O que o gateway precisa fazer

Esta API só decide. As garantias abaixo dependem do gateway que a chama:

- **Aplicar a decisão e falhar fechado.** Trate qualquer resposta diferente de `200` (erro, timeout, conexão recusada) como bloqueio, nunca como liberação. Um caminho com `%`, `?` ou caracteres fora do ASCII recebe `400`: isso é um pedido malformado do gateway, não uma liberação.
- **Normalizar o caminho antes de perguntar.** A comparação com as iscas é exata: `/internal/backup/`, `/Internal/Backup` e `/internal/backup;x=1` não são a isca `/internal/backup`. Decodifique, remova a barra final e aplique a mesma regra de maiúsculas que a aplicação protegida usa.
- **Remover `x-voidgate-key` vindo do cliente.** Se a requisição chegar com dois cabeçalhos, vale o primeiro.
- **Não repassar a resposta ao cliente final.** `reason_code`, `reason_codes`, `findings` e o corpo dos erros dizem qual defesa disparou.
- **Limitar conexões por origem e terminar TLS.** O processo limita o total de conexões, mas não por cliente.
- **Tolerar conexões encerradas.** O servidor fecha conexões ociosas após 30 s e pode fechar a conexão depois de responder `401`, porque não lê o corpo de quem não se autenticou.

## Limites desta etapa

Este serviço não é um firewall de rede nem substitui autenticação da aplicação ou WAF. Não há ainda rate limit distribuído, identidade por tenant, proxy de chat, réplicas, isolamento da isca ou TLS dentro do processo. A decisão de chat só detecta caracteres ocultos; não há redação, detecção de dados pessoais nem de manipulação de prompt. Mantenha o bind local ou coloque a API atrás de um gateway confiável com TLS e controles de acesso. A decisão `allow` ou `decoy` só tem efeito quando esse gateway a aplica.
