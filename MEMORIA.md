# Memória do porte — Nuone Middleware em Rust

Este documento guarda contexto durável e decisões confirmadas para evitar perda de continuidade. Atualize-o quando uma decisão for validada ou quando uma limitação importante mudar.

## Objetivo e direção

- Projeto-alvo: porte do Nuone Middleware 3.0 de Java para Rust.
- Princípio arquitetural: Clean Architecture / Hexagonal, com `domain`, `application`, `adapters` e `infrastructure`.
- O objetivo é preservar comportamento e contratos legados, não converter mecanicamente classes Java.
- A pasta `_reversa_sdd/migration/` não existe mais no projeto original e deve ser ignorada.

## Projetos de referência

- Fonte Java: `C:\Workspace\nuone-3.0\nuone-middleware-3.0`.
- Projeto Rust: `C:\Workspace\nuone-rust\nuone-middleware`.
- A documentação consultada no Java inclui `AGENTS.md`, `README.md`, `PROGRESS.md` e documentação em `docs/`, além do código e testes atuais.

## Fatos observados no projeto Java

- Stack original: Java 21, Maven, Spring Boot 3.3.x, Spring Data JDBC, SQLite, Flyway e OpenTelemetry.
- Camadas existentes: `domain`, `application` (portas in/out e serviços), `adapter` (entrada HTTP e saída para TCP/crypto/protocol/persistence) e `infrastructure`.
- Fluxos HTTP documentados: `/connect`, `/init`, `/menu`, `/command`, `/upload`, `/transaction`, `/api/v1/download`, `/disconnect`, `/close/{sessionId}` e `/agent/status`.
- Compatibilidade de baixo nível sensível: AES HTTP/Base64/ISO-8859-1, XOR TCP, framing e delimitadores PROTO, compactação e traduções JSON ↔ PROTO.
- Sessões mantêm socket ERP e agente de ping; a limpeza precisa ocorrer também em caminhos de falha e shutdown.
- Comandos por sessão são serializados no serviço Java.
- Upload/download têm protocolos e proteções próprias; download restringe origem, redirects e tamanho para reduzir SSRF e exaustão de recursos.
- Logs detalhados podem conter dados sensíveis; modo detalhado é desabilitado por padrão e handshake de autenticação não deve ser persistido.
- O README Java recomenda validar o fluxo real com o ERP antes de ampliar traduções PROTO.

## Divergências conhecidas da documentação Java

- Quantidade de testes declarada diverge: `AGENTS.md` cita 120, `PROGRESS.md` cita 139 e README cita 159.
- Não usar nenhum desses números como baseline validado até executar/verificar a suíte vigente no projeto Java.
- `PROGRESS.md` contém data/estado que pode não refletir os arquivos e testes mais recentes; consultar o código atual antes de assumir completude.

## Decisões para Rust

- [Decidido] Axum + Tokio no crate único; as camadas permanecem módulos internos com fronteiras explícitas.
- [Pendente] Estratégia de persistência e equivalência com SQLite/Flyway.
- [Pendente] Modelo de concorrência para sessões/sockets em Rust.
- Registrar cada decisão com data, motivo, alternativas relevantes e impacto em compatibilidade.

## Estado do porte

- Estado: Etapas 1 e 2 concluídas; Etapas 3 e 4 parcialmente implementadas; Etapa 5 iniciada. Connect/status/disconnect estão compostos e prontos para validação integrada preliminar; `/actuator/health` atende o probe do Gateway. `/init` e `/menu` ainda não estão implementados. A confirmação do baseline Java da Etapa 0 permanece pendente.
- Próximo passo: portar a tradução PROTO → JSON inicial exigida por `/init` e por seleções de item em `/menu`, depois compor os casos de uso e handlers HTTP; validar os fluxos com ERP quando disponível.

## Etapa 1 — esqueleto Rust concluído em 2026-11-16

- Criado crate `nuone-middleware` (versão inicial `0.1.0`), edição 2024, `rust-version = 1.85` e `Cargo.lock`.
- Módulos criados: `domain`, `application` (erro, portas e casos de uso), `adapters` (HTTP, TCP, crypto, protocolo, persistência) e `infrastructure` (configuração e bootstrap).
- Erros de aplicação são tipos explícitos (`ApplicationError`/`ApplicationResult`); configuração exige `cipher` e valida `NUONE_HTTP_BIND`.
- Composição manual no `infrastructure::bootstrap`; o router Axum ainda está vazio e não existem casos de uso ou adapters funcionais.
- `domain` não importa crates externos; `application` usa `thiserror` para erro tipado e não depende de Axum, Tokio ou adapters.
- `.gitignore` protege `target/`, `.env` e arquivos locais de banco. O diretório ainda não foi inicializado como repositório Git.

### Dependências e motivo

- `axum 0.8.9` para roteamento e handlers HTTP; a documentação oficial descreve integração com Tokio/Hyper e suporte a handlers/rotas modulares: https://docs.rs/axum/latest/axum/.
- `tokio 1.54.1` para runtime assíncrono, listener HTTP e futura comunicação TCP; habilitadas apenas features necessárias ao bootstrap e I/O previsto. https://docs.rs/tokio/latest/tokio/.
- `serde`/`serde_json` para contratos JSON; `thiserror` para erros explícitos; `dotenvy` para compatibilidade com `.env`; `tracing`/`tracing-subscriber` para logs estruturados.
- A seleção usa releases disponíveis no cache local e o lockfile fixa a resolução atual; rever crates quando uma necessidade concreta de persistência, crypto ou observabilidade for implementada.

### Validação do esqueleto

- Toolchain: `rustc 1.98.1`, `cargo 1.98.1`.
- `cargo fmt --check`: passou.
- `cargo check --offline`: passou, com dependências já no cache; gerou `Cargo.lock`.
- `cargo clippy --offline --all-targets -- -D warnings`: passou sem avisos.
- Nenhum teste foi adicionado ou executado. `cargo check`/Clippy verificam a estrutura e compilação, não paridade funcional.
- `cargo run` exige `cipher` e inicia um listener com router vazio; não foi necessário iniciar o servidor nesta etapa.

## Etapa 2 — domínio e protocolo básico concluída em 2026-11-16

- Portados `Module`, `Session`, `Ticket`, `TransactionRequest` e `ProtoMessage` para tipos Rust.
- `Session` mantém os estados observados no Java (autenticada, inicializada, foco, ping e modo de serviço), com data de criação `SystemTime`. O socket não pertence ao domínio Rust; ficará sob responsabilidade do adapter/gateway TCP.
- As 110 constantes Java em `ProtoConstants` foram transferidas para `domain::protocol::constants`; a conferência automática de nomes encontrou 110/110, sem ausências ou extras.
- O código-fonte Java contém o marcador de transação U+00A4 (`¤`) seguido de `TS`; ele foi mantido explicitamente como `\u{00a4}TS`. Separador PROTO U+00F7 e os demais caracteres foram preservados como escapes Unicode.
- `encode_proto_text` e `decode_proto_text` implementam ISO-8859-1 real: cada codepoint Latin-1 mapeia para o byte equivalente; caracteres sem representação viram `?`, como na conversão Java usada no projeto. `PROTO_CHARSET` registra o nome do encoding.
- `ProtoParser`/`convert_proto_boolean` preservam as conversões `S`/`.T.` → `true` e `N`/`.F.` → `false`, deixando os demais valores sem alteração.
- `TransactionRequest::to_proto` mantém a ordem `¤TS|customerId|username|password|0|90|transaction`, usando o separador constante; `to_proto_bytes` aplica o encoding compatível.
- Foram adicionados testes Rust para constantes centrais, conversão booleana, limites/replacement de ISO-8859-1, estados da sessão e serialização PROTO da transação em string e bytes.
- A cobertura usa vetores Java selecionados e casos de borda; não valida todos os 110 valores de constantes nem substitui integração com ERP real.

### Validação da Etapa 2

- `cargo fmt --check`: passou.
- `cargo check --offline`: passou.
- `cargo test --offline`: 7 testes passaram; 0 falhas.
- `cargo clippy --offline --all-targets -- -D warnings`: passou sem avisos de código. Cargo informou avisos não fatais de hard-link no cache incremental do filesystem e copiou os arquivos.
- Isso confirma os casos testados localmente; não declarar paridade integrada com o ERP.

## Etapa 0 — levantamento estático em 2026-11-16

### Estado dos repositórios

- Projeto Rust em `C:\Workspace\nuone-rust\nuone-middleware`: só contém `AGENTS.md`, `ROADMAP.md` e `MEMORIA.md`; não há `.git`, `Cargo.toml` nem código-fonte.
- Projeto Java em `C:\Workspace\nuone-3.0\nuone-middleware-3.0`: branch `master`, working tree limpa no commit `8ad6734` (`2026-11-03`).
- Java e Maven não estão no `PATH`, mas foram localizados em `C:\Workspace\Java\openjdk-21.0.12` e `C:\Workspace\apache-maven-3.9.16`; o checkout não inclui `mvnw`/`mvnw.cmd`.

### Inventário Java

- Código-fonte: 29 arquivos em `adapter`, 41 em `application`, 8 em `domain` e 11 em `infrastructure`.
- Testes: 35 classes de teste distribuídas por controllers HTTP, adapters de crypto/protocol/TCP/configuração, serviços de aplicação, domínio e teste de startup.
- Relatórios arquivados em `target/surefire-reports` (14/09/2026): 35 suítes, 159 testes, 0 falhas, 0 erros, 0 ignorados. Isso coincide com o README; `AGENTS.md` Java informa 120 e `PROGRESS.md` informa 139. Os relatórios são anteriores ao HEAD atual (03/11/2026); não houve alterações rastreadas em `src/main/java` ou `src/test/java` depois de 14/09, mas a execução da suíte ainda precisa ser repetida para confirmar o baseline vigente.

### Contratos HTTP observados

| Rota | Request | Response / comportamento |
|---|---|---|
| `POST /connect` | `text/plain`, body criptografado | JSON sem criptografia; inicia sessão ERP. |
| `POST /init` | `text/plain`, body criptografado | JSON `{content:{pack}}`; conteúdo de `pack` criptografado. |
| `POST /menu` | `text/plain`, body criptografado | JSON sem criptografia. |
| `POST /command` | `text/plain`, body criptografado | JSON `{content:{pack}}`; conteúdo de `pack` criptografado. |
| `POST /upload` | `text/plain`, body criptografado | JSON sem criptografia; arquivo em Base64 dentro do comando. |
| `POST /transaction` | `text/plain`, body criptografado | Corpo textual sem criptografia; status depende do resultado. |
| `GET /api/v1/download` | Query `ticket`, `token` e `open` opcional | Bytes do recurso; valida token diário. |
| `POST /disconnect` | `text/plain`, body criptografado | Literal `logout`, sem criptografia. |
| `POST /close/{sessionId}` | Sem body | JSON; encerra a sessão local forçadamente. |
| `GET /agent/status?sessionId=...` | Query `sessionId` | JSON com estado e contadores do ping. |
| `GET /actuator/health` | Sem body | Health check do Spring Actuator; resposta pública básica `{ "status": "UP" }`. |
| `GET /support/integration-logs` | Query opcional e Bearer token | JSON; diagnóstico protegido e configurável. |
| `GET /` | Sem body | JSON de health check legado com versão e sufixo do modo. |

- Handlers mapeiam erros de domínio `400`, `401` e `404` para HTTP correspondentes; outros códigos viram `500`. `/transaction` tem tratamento próprio e resposta textual.
- Os requests de sessão aceitam `sessionId` e `sessionID` nos pontos em que o código os trata explicitamente. `/transaction` não usa sessão interativa.
- O Java depende de `spring-boot-starter-actuator` e expõe `/actuator/health`; o Rust implementa a resposta básica `{"status":"UP"}` sem detalhes para o health probe do Gateway. Os endpoints Actuator de métricas e info não estão neste escopo.
- `/init`, `/command` e `/menu` devolvem dados traduzidos/encapsulados diferentes; não presumir um envelope único para todos os endpoints.

### Vetores de paridade já localizados

Fontes: testes Java versionados em `src/test/java/...` e constantes em `src/main/java/.../ProtoConstants.java`.

| Área | Entrada de referência | Saída/contrato esperado |
|---|---|---|
| AES HTTP | Plaintext `{"command":"PING","acentos":"acao"}`, chave `1234567890123456` | Base64 `4DJRDPh8hmf0NWWUBd9x2tz5rgDh/pwmhvT284nTvY1P+A0sHua4B4FxAeUGfN8t`; AES/ECB/PKCS5Padding e texto ISO-8859-1. |
| XOR TCP | Bytes ISO-8859-1 de `PROTO` | `[21, 23, 10, 17, 10]`; chave XOR 69, operação reversível. |
| TCP handshake | Mensagem `HELLO` | `XONGAS÷?0÷HELLO¬¹²³` (separador `÷`, finalizador `¬¹²³`). |
| TCP framing | `SESSION÷OK¬¹²³` | Receber até o finalizador e desfazer XOR; envelope comprimido também encerra leitura por `$<ENDPACK>$`. |
| Packer | Mensagem curta `short-proto-message` | Permanece sem compressão. Acima de 200 bytes usa `$<PACK>$`, `$<ENDPACK>$`, comprimento e `$<ENDLEN>$`; round-trip preserva bytes. |
| JSON → PROTO | `{"command":{"click":"NButton","parent":"105","id":"6"}}` | `ø÷µ÷105÷6` em ISO-8859-1 (`ø` = `PROTO_W_DLGEVENT`, `µ` = `PROTO_EVENT_CLICK`, `÷` = separador). |
| JSON → PROTO | `UpdateComponent` com newline | Newline normalizado para CRLF; exemplos adicionais cobrem menuitem, checkbox, toolbar, grid/search, `ClientData` e comandos não suportados. |

- Referências de testes: `AesCryptoAdapterTest`, `XorCryptoAdapterTest`, `TcpCommunicationTest`, `DeflatePackerAdapterTest`, `JsonToProtoTranslatorTest` e `ProtoToJsonTranslatorTest`.
- Não foi feita captura nova do ERP; constantes/protocolos não cobertos por vetor fixo devem ser comparados com o código e fixtures existentes antes de implementar.

### Tentativa de execução do baseline

- Em 2026-11-16 foi iniciada a suíte com Java 21.0.12 e Maven 3.9.16. O Maven confirmou que fontes/classes estavam atualizadas, iniciou JUnit e progrediu até `LegacyFileTransferAdapterTest`, mas permaneceu sem saída por mais de um minuto; a execução foi interrompida sem resumo final.
- O teste isolado `mvn -DforkCount=0 -Dtest=LegacyFileTransferAdapterTest test` também parou ao iniciar o mesmo caso. O teste usa `ServerSocket` em loopback e espera até dois segundos pelo futuro; a causa externa ou interna do travamento ainda não foi determinada.
- A mensagem `Access is denied` apareceu no início de ambas as invocações Maven, embora o Maven tenha iniciado o build. Investigar se afeta sockets locais ou processos forkados.
- Os relatórios antigos em `target/surefire-reports` não foram substituídos e continuam sendo apenas evidência histórica de 159 testes aprovados.

### Pendências após o levantamento

- Reexecutar `mvn test` em ambiente com Java 21 e Maven 3.9+ para transformar o relatório arquivado em baseline atual.
- Confirmar os contratos integrados com front-end/ERP em ambiente real; este levantamento foi estático.
- Definir se o diretório será inicializado como Git antes do primeiro código e escolher runtime/framework/crates Rust na Etapa 1.

## Etapa 3 - crypto, packer e TCP parcial em 2026-11-16

- Criadas portas `CryptoPort` e `PackerPort` em `application::ports`, com erros explícitos e sem tipos de framework.
- `CryptoAdapter` implementa AES/ECB com padding PKCS#7 (equivalente ao PKCS5Padding do Java em AES), Base64 padrão e texto ISO-8859-1; a chave usa UTF-8, equivalente a `String.getBytes()` no Java 21 padrão.
- Vetor fixo AES do teste Java aprovado exatamente: plaintext `{"command":"PING","acentos":"acao"}`, chave `1234567890123456`, saída Base64 `4DJRDPh8hmf0NWWUBd9x2tz5rgDh/pwmhvT284nTvY1P+A0sHua4B4FxAeUGfN8t`.
- O mesmo adapter implementa XOR byte a byte com chave 69. Para ISO bytes `PROTO`, o resultado fixo é `[21, 23, 10, 17, 10]`.
- `DeflatePackerAdapter` preserva o limiar (mensagens até 200 bytes sem compressão), limite de empacotamento de 10 MiB e envelope `$<PACK>$` + raw DEFLATE + `$<ENDPACK>$` + tamanho decimal + `$<ENDLEN>$`. O desempacotador preserva o comportamento observado de localizar `$<ENDPACK>$` e ignorar o comprimento declarado.
- `TcpCommunication` oferece conexão e I/O assíncronos via Tokio, XOR na borda, mensagens de handshake/sessão, timeouts por conexão/leitura/escrita, EOF explícito e parada de leitura no finalizador PROTO ou no marcador de fim do packer. Streams são ownership do chamador e liberados por RAII ao descartar.
- Opções mapeadas: connect/socket timeout, tamanhos dos buffers, keep-alive, TCP no-delay, linger e traffic class IPv4. `setPerformancePreferences(0,1,2)` do Java não tem equivalente direto no Tokio; traffic class IPv6 ainda não foi mapeada pela API usada. Essas diferenças mantêm a Etapa 3 aberta.
- Foram geradas fixtures raw-DEFLATE dos dois runtimes para `PROTO_FIELD;` repetido 30 vezes. Java produziu `CwjyD/GPd/N09XGxDhhl04wNAA==`; Rust produziu `7cbBCQAABAXQwVxRSv2f3O2/hSnc3B4LjfGwVOH7zAs=`. Os bytes compactados diferem, mas Rust descompactou a fixture Java em teste e Java 21 descompactou a saída Rust com sucesso. Portanto, a compatibilidade do packer é semântica/transportável, não identidade dos bytes comprimidos.
- Os testes atuais verificam vetor AES, XOR, envelope/round-trip Rust, fixture Java→Rust, erro de conteúdo comprimido inválido, framing, envio, EOF e timeout. Também não houve teste contra ERP real.
- Dependências adicionadas: `aes 0.8.4`, `base64 0.22.1` e `flate2 1.1.10`; as versões foram resolvidas do cache offline e fixadas por `Cargo.lock`. O backend DEFLATE do crate precisa manter interoperabilidade raw-deflate do `Deflater(..., true)` Java.

### Validação da Etapa 3 parcial

- `cargo test --offline -j 1`: 20 testes passaram; 0 falhas. O `-j 1` evitou falha intermitente do linker MSVC ao sobrescrever o executável de teste.
- `cargo clippy --offline --all-targets -- -D warnings`: passou.
- `cargo fmt --check`: passou.
- Avisos restantes durante compilação são de hard-link do cache incremental no filesystem; Cargo copia os arquivos e os comandos terminam com sucesso.
- Esses resultados são locais e não demonstram paridade ponta a ponta com ERP ou execução completa da suíte Java.

## Etapa 4 - sessão e autenticação parcial em 2026-11-17

- Criado application::ports::session com contratos para BotGateway, SessionRepository e SessionKeepAlivePort; erros tipados ficam nas fronteiras.
- InMemorySessionRepository armazena snapshots de Session atrás de Mutex<HashMap<...>>. O modelo Rust continua sem Socket ou identificador de conexão; a conexão física e o mapa sessionId para gateway connection pertencem ao adapter TCP.
- TcpBotGateway guarda TcpStreams por identificador opaco, aplica os envelopes handshake/sessão e mantém o mapa sessionId para conexão. Operações de I/O são protegidas por mutex do stream; disconnect sinaliza cancelamento para interromper leitura/escrita em andamento.
- AuthenticationService implementa a sequência version -> init -> start -> logon -> sendOs, substituição de sessão pelo mesmo username e rollback de conexão quando handshake/registro/keepalive falha. A sessão é persistida antes do vínculo sessionId para evitar substituir uma conexão ativa se o ERP reutilizar um identificador já registrado. AuthenticationCommand não deriva Debug, evitando exposição acidental da senha em logs.
- A validação usa AuthenticationCommand do caso de uso e um gateway fake. Cenários cobertos: autenticação, persistência do estado autenticado, rejeição de credenciais com desconexão/sem sessão persistida, e request/servidor inválido antes da conexão.
- SessionLifecycleService contém fluxos de disconnect normal (envia PROTO_NORMAL_CLOSE, exceto em transação) e force-close best-effort com remoção local; há testes para os dois fluxos e para erro de disconnect remoto.
- LegacyPingAgent agenda pings Tokio com atraso fixo após cada tentativa, envia PING {sessionId} HTTP/1.0\r\n, lê até HTTPEAS/ ou EOF, mantém contadores/horários/último erro, permite cancelamento da tarefa e aborta tarefas no drop. Defaults observados no Java: ativo, porta 4081, intervalo 60000 ms, connect/read timeout 5000 ms. Configuração por ambiente e endpoint /agent/status foram compostos; ainda sem teste de rede real.
- Gap de compatibilidade: sendOs Java envia System.getProperty("os.name"); Rust envia std::env::consts::OS (por exemplo, windows). O texto exato ainda deve vir de configuração/port de plataforma antes da paridade do handshake ser declarada.
- Gap de paridade TCP anterior permanece: traffic class IPv6 não é exposta na API Tokio usada e setPerformancePreferences não possui equivalente direto.
- Infraestrutura inicial passou a compor CryptoAdapter, TcpCommunication, TcpBotGateway, repositório em memória, LegacyPingAgent, casos de uso e rotas HTTP /connect, /disconnect, /close/{sessionId}, /agent/status, /, e /actuator/health.
- Para os próximos casos de uso por sessão, `BotGateway` agora expõe `receive_session(sessionId)`; `Settings` lê `NUONE_INIT_LOOP_TIMEOUT_MS` (default 30000 ms, conforme `AppProperties.initLoopTimeoutMs` Java) e o `.env` local declara esse valor.
- Iniciada a porta `ProtoToJsonPort` e o `ProtoToJsonAdapter` com mapeamentos Java conferidos para MessageBox, Alert, Window/ModalWindow, Label/LabelActive e Button. Respostas ACK e server-version são ignoradas como no Java; blocos sem mapeamento também são omitidos. Ainda faltam os demais componentes/eventos; o adapter ainda não está conectado a `/init` ou `/menu`.
- Configuração valida cipher com comprimento AES de 16/24/32 bytes e lê os timeouts/opções TCP, ping, bot version e mode suffix usados nesta fatia.

### Validação da Etapa 4 parcial

- cargo test --offline -j 1: 26 testes passaram; 0 falhas.
- cargo clippy --offline --all-targets -- -D warnings: passou.
- cargo fmt --check: passou.
- Os testes da autenticação e ciclo de vida usam portas fake; não há validação contra socket ERP real, ping real, shutdown concorrente ou sessão substituída.
- A aplicação está pronta para um teste integrado preliminar do ciclo connect -> status -> disconnect. Esse fluxo ainda não foi executado; /init e /menu continuam fora do marco.

## Registro de decisões e validações

| Data | Decisão ou validação | Evidência / impacto |
|---|---|---|
| 2026-11-16 | Ignorar `_reversa_sdd/migration/`; a pasta foi removida após a conclusão do projeto Java. | Confirmação do usuário. |
| 2026-11-16 | Base Rust em crate único com Axum + Tokio; módulos de Clean Architecture ficam no mesmo crate até surgir motivo concreto para separação. | Axum atende roteamento HTTP; Tokio fornece runtime e I/O TCP assíncronos. Nenhuma dependência de framework foi adicionada ao domínio/aplicação. |
| 2026-11-16 | `rust-version = 1.85`, edição 2024 e lint `unsafe_code = forbid`. | Compatibilidade com MSRV exigido pelo Tokio 1.54 e toolchain local 1.98.1. |
| 2026-11-16 | Crypto legado em `aes 0.8.4` + `base64 0.22.1`; compactação em `flate2 1.1.10`. | Vetor AES Java idêntico; raw-DEFLATE cruza entre Java 21 e Rust, apesar de os bytes comprimidos não serem iguais. |
| 2026-11-17 | async-trait 0.1.92 para portas assíncronas em application sem expor Tokio. | BotGateway é async; tipos de socket e runtime permanecem somente nos adapters. |
