# Instruções para agentes — Nuone Middleware em Rust

## Objetivo

Portar o middleware Java Nuone 3.0 para Rust, preservando os contratos HTTP e o comportamento validado com o ERP legado. Esta migração deve manter os princípios de Clean Architecture / Hexagonal e usar Rust de forma idiomática; não deve ser uma tradução mecânica de classes Java.

## Fontes de referência

- Projeto Java de referência: `C:\Workspace\nuone-3.0\nuone-middleware-3.0`.
- Consulte o código, os testes e a documentação que existem atualmente nesse projeto.
- A pasta `_reversa_sdd/migration/` foi removida do projeto original e não é uma dependência desta migração.
- Leia `ROADMAP.md` para as etapas planejadas e `MEMORIA.md` para decisões e fatos consolidados. Atualize a memória quando uma decisão de arquitetura ou compatibilidade for validada.

## Princípios arquiteturais

- Mantenha as fronteiras `domain`, `application`, `adapters` e `infrastructure`.
- `domain` contém modelos, regras e parsing de protocolo sem dependência de HTTP, banco, runtime ou framework.
- `application` contém casos de uso e portas (`trait`s). Os casos de uso dependem das abstrações, nunca das implementações concretas.
- `adapters` implementam portas para HTTP, TCP, criptografia, PROTO, persistência e armazenamento de arquivos.
- `infrastructure` configura e conecta as implementações; mantenha a composição de dependências explícita.
- Não coloque regras de negócio, protocolo, criptografia ou framing TCP nos handlers HTTP.
- Não introduza atalhos entre camadas para acelerar o porte. Prefira estender uma porta ou adapter existente.
- Evite abstrações genéricas sem necessidade concreta; preserve limites simples e verificáveis.

## Compatibilidade e comportamento

- Preserve contratos existentes de endpoints, formatos JSON, códigos/respostas observáveis e aliases legados (`sessionId` e `sessionID` quando aceitos).
- Trate criptografia HTTP, codificação ISO-8859-1, XOR, framing TCP, delimitadores, compactação e mensagens PROTO como contratos byte a byte.
- Não altere constantes, schema de persistência nem semântica de sessão sem evidência no código/testes de referência e justificativa registrada.
- Implemente traduções JSON ↔ PROTO em fatias pequenas. Cada novo caso deve ter exemplos e testes focados.
- Preserve o ciclo de vida: sockets, sessão e agentes de ping precisam ser limpos em falhas, disconnect, substituição de login, encerramento forçado e shutdown.
- Preserve as proteções de upload/download, incluindo validação de nomes, limites de tamanho e restrições contra SSRF.
- Handshakes e credenciais nunca devem ser gravados em logs. Logs detalhados que possam conter payloads ficam desativados por padrão.
- Não invente comportamento para casos ainda não confirmados. Registre dúvidas e limites na `MEMORIA.md`.

## Convenções de implementação em Rust

- Use `Result` e tipos de erro explícitos nas fronteiras; não esconda falhas com `unwrap()`/`expect()` em caminhos de runtime.
- Use ownership e tipos para representar claramente estados e limites; recorra a `Arc`, locks e canais somente quando o compartilhamento exigir.
- A comunicação TCP e o servidor HTTP devem tratar concorrência sem quebrar a serialização das operações por sessão.
- Evite `unsafe`; qualquer uso precisa ser estritamente necessário, pequeno e documentado.
- Mantenha configuração e segredos externos ao código e valide a configuração no startup.
- Escolha frameworks e crates com base nos requisitos observados e registre a escolha na `MEMORIA.md`; não deixe tipos de framework vazarem para `domain` ou `application`.

## Testes e validação

- Adicione testes focados para cada mudança de comportamento e use vetores de bytes fixos para protocolo e criptografia.
- Cubra domínio e casos de uso sem rede real, usando implementações de teste das portas.
- Teste adapters de TCP e HTTP nos limites observáveis, incluindo erros e limpeza de recursos.
- Use o projeto Java e seus testes como referência de paridade; valide os contratos de ponta a ponta contra o ERP quando o ambiente estiver disponível.
- Não declare paridade com base apenas em compilação ou em testes unitários isolados.
- Antes de publicar um número de testes de referência, confira o baseline real: a documentação Java atualmente diverge (AGENTS: 120; `PROGRESS.md`: 139; README: 159).

## Escopo de mudanças

- Faça mudanças pequenas, revisáveis e alinhadas ao estágio atual em `ROADMAP.md`.
- Atualize `MEMORIA.md` com decisões confirmadas, desvios justificados e limitações relevantes.
- Não altere o projeto Java de referência durante o porte Rust, a menos que isso seja solicitado explicitamente.
