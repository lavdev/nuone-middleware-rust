# Roadmap — porte do Nuone Middleware para Rust

## Objetivo

Reimplementar o middleware Nuone 3.0 em Rust seguindo Clean Architecture / Hexagonal e preservando contratos e comportamento compatível com o ERP legado.

O roadmap é incremental. Não avançar uma área de protocolo sensível em bloco: cada etapa deve produzir código compilável, testes focados e evidência de compatibilidade.

## Etapa 0 — Preparação e baseline

- [x] Confirmar a raiz e o estado do projeto Rust: contém somente `AGENTS.md`, `ROADMAP.md` e `MEMORIA.md`; ainda não tem `.git`, `Cargo.toml` ou código-fonte.
- [x] Inventariar os módulos, endpoints e testes existentes no projeto Java de referência.
- [ ] Confirmar o baseline vigente executando a suíte Java. Relatórios Surefire arquivados somam 159 testes em 35 suítes, todos aprovados, e coincidem com o README. A execução atual foi tentada com Java 21/Maven 3.9.16, mas travou ao iniciar `LegacyFileTransferAdapterTest`; não foi obtido resumo JUnit.
- [x] Catalogar os contratos HTTP, formatos de request/response, mapeamento de erros e aliases observados no código/documentação.
- [x] Localizar vetores verificáveis para AES, XOR, framing TCP, packer e traduções PROTO nos testes existentes.
- [x] Registrar decisões, limites da evidência e próximos passos em `MEMORIA.md`.

### Resultado e limite da Etapa 0

O levantamento estático está concluído, mas a Etapa 0 permanece aberta até confirmar o baseline em execução. O número de 159 testes vem de relatórios arquivados em `target/surefire-reports` (14/09/2026), não de uma execução atual concluída. O commit Java atual é de 03/11/2026; desde os relatórios, não houve alterações rastreadas em `src/main/java` ou `src/test/java`. Foram tentadas a suíte completa e a execução isolada de `LegacyFileTransferAdapterTest`, inclusive sem fork; ambas pararam ao chegar ao teste de socket local, sem resumo ou relatório novo, e foram interrompidas. O teste isolado espera até dois segundos pela resposta; a causa do travamento não foi determinada. Reexecute em um ambiente onde o teste de socket local consiga completar. Nenhum acesso ao ERP real foi feito.

## Etapa 1 — Esqueleto Rust e composição

- [x] Criar o crate e configurar lint, formatação e comandos Cargo.
- [x] Estabelecer módulos `domain`, `application`, `adapters` e `infrastructure`.
- [x] Definir tipos básicos de erro e validar `cipher` e `NUONE_HTTP_BIND` na configuração.
- [x] Criar composição explícita no bootstrap, sem container de DI.
- [x] Validar que `domain` e `application` não dependem de Axum ou adapters concretos.

### Limite do esqueleto

O crate sobe um servidor HTTP com router vazio. Nenhum endpoint, caso de uso ERP, socket legado ou comportamento funcional foi portado nesta etapa. A pendência de baseline Java da Etapa 0 continua aberta em paralelo.

## Etapa 2 — Domínio e protocolo básico

- [x] Portar modelos de domínio e estados de sessão necessários.
- [x] Portar constantes PROTO e encoding ISO-8859-1 compatível.
- [x] Portar parsing e conversões booleanas observadas.
- [x] Criar testes de paridade com vetores Java selecionados e casos limite de encoding.

Os quatro itens foram concluídos para os modelos e o protocolo básico. A cobertura testa vetores selecionados; não constitui paridade integrada com o ERP.

## Etapa 3 — Adapters binários e TCP

- [x] Implementar porta de criptografia e adapter AES/Base64 conforme o contrato legado.
- [x] Implementar XOR e o packer/deflate com limites e marcadores preservados.
- [ ] Implementar conexão TCP, opções de socket, framing e leitura até finalizador/envelope compactado. A comunicação, framing e opções principais foram implementados; falta definir/aplicar traffic class em IPv6 e revisar a dica `setPerformancePreferences` do Java, sem equivalente direto no Tokio.
- [x] Garantir timeout por conexão/leitura/escrita, tratamento de EOF e liberação por ownership/RAII.
- [x] Verificar vetores fixos para AES, XOR, envelope do packer e framing TCP, além de fixtures cruzadas raw-DEFLATE nos dois sentidos.

### Estado parcial da Etapa 3

Crypto, XOR, packer, framing e testes focados estão implementados. As streams raw-DEFLATE diferem byte a byte entre os compressores Java e Rust, mas cada runtime descompactou com sucesso a saída do outro. A etapa permanece aberta para fechar as diferenças de opções TCP IPv6 e dica de performance; a suíte Java completa e o ERP real também continuam pendentes.

## Etapa 4 — Sessão e autenticação

- [x] Implementar portas de gateway ERP, repositório de sessão e keep-alive, com adapters TCP/memória.
- [x] Portar handshake `version`, `init`, `start`, `logon` e `sendOs`; fluxo feliz e rollback por credenciais inválidas têm testes com gateway fake.
- [ ] Validar exclusividade/substituição de login e limpeza em todos os caminhos de falha. Fluxos principais estão implementados; ainda faltam testes de substituição concorrente, falha do ping e limpeza em shutdown. Disconnect normal, disconnect transacional e force-close com falha remota já têm testes focados.
- [x] Implementar ping legado configurável e endpoint de status. Configuração e status foram conectados por ambiente e HTTP; faltam testes de rede real, falha e shutdown.
- [ ] Testar todos os caminhos de falha e encerramento de recursos.

### Estado parcial da Etapa 4

O Session Rust não guarda Socket nem handle do gateway; o adapter TCP mantém conexões e vínculo por sessionId. O handshake usa os mesmos tokens/campos e a limpeza após rejeição de credenciais fecha a conexão. O valor enviado em sendOs usa std::env::consts::OS; isso ainda precisa ser alinhado ao System.getProperty("os.name") Java ou configurado antes de declarar paridade byte a byte. A composição no bootstrap e os endpoints HTTP ficam para etapas seguintes.

## Etapa 5 — Endpoints básicos e ciclo da sessão

- [x] Implementar `/connect` e `/disconnect` com body text/plain criptografado e respostas text/plain conforme Java.
- [x] Implementar `/init` e `/menu`, incluindo suas diferenças de envelope/cripto e serialização por sessão.
- [x] Preparar recebimento TCP por `sessionId` e configurar `NUONE_INIT_LOOP_TIMEOUT_MS` (padrão Java: 30000 ms).
- [x] Implementar `/close/{sessionId}`, `/agent/status`, `GET /` e `GET /actuator/health`.
- [ ] Validar todos os contratos HTTP (aliases, erros e formatos JSON) em testes de adapter.
- [ ] Testar validação, aliases de campos, erros e compatibilidade dos contratos HTTP.

### Ponto de teste alcançado

O fluxo HTTP principal já pode ser exercitado contra um ERP de teste ou real: POST /connect, POST /init, POST /menu para listagem/navegação/execução e POST /disconnect. A implementação foi compilada, mas esse teste integrado ainda não foi executado; ausência de teste não significa paridade comprovada.

## Etapa 6 — Traduções e execução de comandos

- [x] Iniciar a porta PROTO → JSON com adaptação de MessageBox, Alert, Window/ModalWindow, Label/LabelActive e Button.
- [x] Ampliar PROTO → JSON para WaitRun, refresh, window-close, atualização de grid, enable/visible/hide, SetText, foco, ClientData e eventos de status.
- [x] Portar os handlers de componentes e eventos registrados no tradutor Java, incluindo campos, combos, checkbox, grid, imagem, caixa, toolbar, fonte, cor e ícone; compilação validada.

`PROTO → JSON` é dependência de `/init` e do ramo que executa item em `/menu`; listagens de módulos têm formato próprio.

- [x] Implementar primeira versão da tradução JSON para PROTO cobrindo os ramos registrados no tradutor Java; compilação validada, paridade por testes pendente.
- [ ] Implementar PROTO → JSON com dispatch por registries de componentes/eventos ou mecanismo equivalente claro.
- [ ] Preservar serialização de comandos por sessão.
- [ ] Implementar `/command`, incluindo materialização de links de download quando aplicável.
- [ ] Adicionar testes de paridade por comando, componente, evento e resposta desconhecida.

## Etapa 7 — Upload, download e transações

- [ ] Implementar transferência dedicada de arquivos e confirmação pela sessão ERP.
- [ ] Implementar tickets, armazenamento/cache e `/api/v1/download`.
- [ ] Preservar restrições contra path traversal, SSRF, redirects e excesso de bytes.
- [ ] Implementar `/transaction`, denylist e fluxo TCP dedicado.
- [ ] Testar erros de rede, limites, encerramento de sockets e conteúdo binário.

## Etapa 8 — Operação e observabilidade

- [ ] Portar configuração por ambiente e validação de startup.
- [ ] Definir logging seguro, métricas, tracing e health checks equivalentes aos contratos operacionais. `GET /actuator/health` foi portado para o Gateway; métricas e outros endpoints Actuator continuam pendentes.
- [x] Implementar logs de payload HTTP em modo de desenvolvimento, desligados por padrão e com sanitização recursiva de credenciais.
- [ ] Implementar endpoint de diagnóstico protegido se permanecer no escopo.
- [ ] Documentar execução local, variáveis, persistência e operação.

## Etapa 9 — Paridade integrada e corte

- [ ] Executar suíte completa de testes Rust e comparar respostas com fixtures do Java.
- [ ] Validar fluxo real front-end → middleware → ERP: connect, init, menu, comandos, upload, download e disconnect.
- [ ] Verificar concorrência, timeout, shutdown e recuperação de falha.
- [ ] Registrar gaps conhecidos e evidência de paridade em `MEMORIA.md`.
- [ ] Definir estratégia de implantação/corte somente após a validação integrada.

## Critério para concluir uma etapa

Uma tarefa pode ser marcada como concluída quando houver implementação revisável, teste focado, resultado registrado e nenhum desvio de contrato sem justificativa. Funcionalidade que dependa do ERP real deve permanecer identificada como aguardando validação integrada.
