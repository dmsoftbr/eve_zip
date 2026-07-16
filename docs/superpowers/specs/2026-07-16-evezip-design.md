# EveZip — Design v1

**Data:** 2026-07-16
**Status:** Aprovado
**Objetivo:** App desktop multiplataforma (macOS/Linux/Windows) de compactação/descompactação com UI amigável estilo 7-Zip File Manager, para uso interno da dmsoft.

## Decisões de produto

| Decisão | Escolha |
|---|---|
| Público | Uso interno da empresa |
| Formatos — criação | zip, 7z, tar, gz (tar.gz) |
| Formatos — extração | zip, 7z, rar, tar, gz (RAR é somente-leitura: criar RAR exige licença RARLAB) |
| Modelo de UI | Gerenciador estilo 7-Zip: janela única que navega pelo sistema de arquivos e entra em archives como se fossem pastas |
| Stack | Rust + Slint (UI) + binário oficial `7zz` embutido como motor |
| Recursos v1 | Senha/criptografia AES-256, fila de tarefas com progresso e cancelamento, preview via app padrão do SO, associação de arquivos + CLI |

## Stack e justificativa

- **Rust** — equipe domina Rust/C/C++; segurança de memória; cargo como build system único nas 3 plataformas.
- **Slint** — UI declarativa com renderer próprio, licença royalty-free para desktop, binário leve e startup instantâneo. Limitação aceita: drag de arquivos para FORA da janela (soltar no Finder/Explorer) é limitado — mitigado com "Extrair para..." e extração rápida.
- **Motor `7zz`** (7-Zip oficial, LGPL, com codec unRAR incluso) conduzido por subprocess — mesmo modelo do Keka (macOS). Dá criação zip/7z/tar/gz, extração de todos os formatos incluindo RAR, AES-256 e comportamento idêntico nas 3 plataformas, sem reimplementar formatos. Binários vendorizados por plataforma em `vendor/7zz/`.

Alternativas descartadas: Qt 6 Widgets/C++ (mais maduro para file manager, porém equipe preferiu Rust); Flutter + Rust core (exigiria aprender Dart, app ~100MB, drag-out precário); bibliotecas nativas de compressão (colcha de retalhos com maturidade desigual entre formatos).

## Arquitetura

Workspace Rust com 3 crates:

```
evezip/
├── crates/
│   ├── evezip-engine/   # wrapper do 7zz (subprocess)
│   ├── evezip-core/     # domínio: árvore de archive, fila de jobs, config
│   └── evezip-ui/       # Slint: janela, diálogos, view-models
└── vendor/7zz/          # binários oficiais por plataforma (mac/linux/win)
```

### evezip-engine

Única camada que conhece o 7zz. Responsabilidades:

- Localizar o binário vendorizado da plataforma corrente.
- Montar linhas de comando e spawnar subprocess para as operações:
  - `list` — `7zz l -slt <archive>` → parse para lista de entradas (caminho, tamanho, tamanho comprimido, data de modificação, flag de diretório, flag de criptografado).
  - `extract` — `7zz x -bsp1 -o<dest> <archive> [seleção]` → stream de progresso em %.
  - `create` — `7zz a -bsp1 [-p... -mhe=on] <archive> <inputs>` → stream de progresso. Para tar.gz: `tar` + `gzip` em dois passos via 7zz.
  - `test` — `7zz t <archive>`.
- Traduzir saída/exit codes em eventos e erros tipados: `SenhaNecessaria`, `SenhaIncorreta`, `ArchiveCorrompido`, `SemEspacoEmDisco`, `Cancelado`, `ErroDesconhecido(stderr)`.
- Cancelamento = kill do subprocess.

Interface: funções síncronas bloqueantes que emitem eventos por callback/channel; o chamador decide o threading. Nenhuma dependência de UI.

### evezip-core

- **Fila de tarefas:** jobs com estados `NaFila → Executando → Concluído | Erro | Cancelado`. Cada job roda num worker thread e publica eventos (progresso, término, erro) por channel.
- **Árvore de archive:** ao abrir um archive, chama `engine::list` uma vez e monta árvore de entradas em memória para navegação virtual (sem extrair nada).
- **Caminho unificado:** conceito de "caminho atual" que aponta para um diretório do disco OU para um caminho dentro de um archive (`~/Downloads/backup.7z/src/`). `..` na raiz do archive volta ao disco.
- **Config:** preferências simples (última pasta, colunas) em arquivo TOML no diretório de config da plataforma.
- 100% testável sem UI.

### evezip-ui (Slint)

- **Janela principal:** toolbar (Adicionar, Extrair, Testar, Info), breadcrumb do caminho atual, tabela virtualizada (Nome, Tamanho, Comprimido, Modificado), status bar (itens/seleção).
- **Painel de fila:** progresso por job, botão de cancelar, erro expansível.
- **Diálogos:** criar archive (formato, nível de compressão, senha, criptografar nomes no 7z), prompt de senha na extração (com reprompt em senha errada), confirmação de sobrescrita.
- Threading: Slint no thread principal; eventos dos workers chegam via `slint::invoke_from_event_loop`.

## Fluxos principais

**Extração:** UI → core cria Job → engine spawna `7zz x -bsp1` → parse do stdout (%) → evento por channel → UI atualiza barra. Ao final, opção "abrir pasta de destino".

**Navegar em archive:** duplo-clique num `.7z` na tabela → core chama `list`, monta árvore → breadcrumb vira `.../backup.7z/` → navegação local na árvore em memória.

**Preview:** duplo-clique num arquivo dentro do archive → extrai só ele para diretório temporário → abre com o app padrão (`open` / `xdg-open` / `start`). Temp limpo ao fechar o app.

**Criação:** seleção de arquivos no disco → "Adicionar" → diálogo (formato/nível/senha) → job na fila.

**Senha:** extração falha com `SenhaNecessaria`/`SenhaIncorreta` → prompt → re-tenta o job com a senha informada.

## Integração com o SO (v1)

- **Associação de arquivos:** duplo-clique em `.zip/.7z/.rar/.tar/.gz/.tar.gz` abre no EveZip — `Info.plist` (macOS), registro via instalador (Windows), `.desktop` (Linux).
- **CLI:** `evezip <archive>` abre a janela no archive; `evezip x <archive> [-o dest]` extrai sem UI (base para automações e futuros menus de contexto).
- **Fora do escopo v1 (fica para v1.1):** menus de contexto "Extrair aqui" no Finder/Explorer/Nautilus — cada SO exige mecanismo próprio e não justificam atrasar a v1.

## Tratamento de erros

- Erros do 7zz mapeados para mensagens amigáveis em português; nunca despejar stderr cru no usuário (stderr fica no detalhe expansível do job e em log em disco).
- Senha errada → reprompt; archive corrompido → sugerir "Testar"; disco cheio → mensagem clara com caminho de destino.
- Subprocess que morre inesperadamente → job marcado como erro com detalhe.

## Testes

- **Unitários (prioridade):** parser de `-slt` e parser de progresso `-bsp1` — os pontos mais frágeis do sistema; casos com nomes unicode, caminhos profundos, archives criptografados.
- **Integração:** engine contra archives-fixture reais (zip/7z/rar/tar.gz, com e sem senha) gerados no CI; roda nas 3 plataformas.
- **Core:** fila de jobs (cancelamento, erro, concorrência) com engine falso (mock).
- UI: lógica mantida fina; sem testes automatizados de UI na v1.

## Distribuição (uso interno)

- macOS: `.dmg` com assinatura ad-hoc (Gatekeeper: liberar na 1ª execução; notarização se/quando houver conta Apple Developer).
- Windows: `.zip` ou `.msi` simples.
- Linux: AppImage.
- Binário do `7zz` correspondente empacotado junto em todas as plataformas.

## Fora do escopo da v1

- Criação de RAR (licença proprietária RARLAB).
- Menus de contexto do SO ("Extrair aqui") — v1.1.
- Viewer de preview embutido (texto/imagem dentro do app).
- Drag de arquivos para fora da janela (limitação do Slint; reavaliar quando o suporte evoluir).
- Edição in-place dentro de archives (abrir, editar, re-compactar automático).
