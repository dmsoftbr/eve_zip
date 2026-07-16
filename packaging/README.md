# Empacotamento e associação de arquivos — EveZip

Instruções de empacotamento e instalação por plataforma. Uso interno (v1);
não há assinatura Apple/Microsoft nem instalador gráfico — a instalação é
manual, mas suficiente para distribuir a build entre máquinas confiáveis.

## macOS

1. Rodar o script de bundle a partir da raiz do repositório:

   ```bash
   chmod +x scripts/package-macos.sh
   ./scripts/package-macos.sh
   ```

   O script builda o release (`cargo build --release -p evezip-ui`), garante
   o `7zz` vendorizado (`scripts/fetch-7zz.sh`), monta `target/EveZip.app`
   (com `Contents/MacOS/evezip` e `Contents/MacOS/7zz` lado a lado — o motor
   localiza o `7zz` pelo "diretório do executável", ver
   `crates/evezip-engine/src/locate.rs`), assina ad-hoc (`codesign --sign -`)
   e gera `target/EveZip.dmg` via `hdiutil`.

2. Instalar: abrir o `.dmg` e arrastar `EveZip.app` para `/Applications`.

3. Primeira execução: como a assinatura é ad-hoc (não há certificado de
   desenvolvedor Apple), o Gatekeeper bloqueia o duplo-clique direto na
   primeira vez. Usar clique-direito → **Abrir** e confirmar no diálogo.
   Nas execuções seguintes o duplo-clique funciona normalmente.

4. Associação de tipos de arquivo: o `Info.plist` já declara
   `CFBundleDocumentTypes` para `zip`, `7z`, `rar`, `tar`, `gz`, `tgz`. Na
   primeira vez, clicar com o botão direito num arquivo desses tipos →
   **Abrir com** → **EveZip** → **Sempre abrir com esta aplicação**.

## Linux

Não há AppImage nesta versão (ver divergência de escopo abaixo). Instalação
manual via tarball:

1. Copiar o binário `evezip` (release) e o `7zz` vendorizado para
   `/opt/evezip/` (mesmo diretório, para o motor localizar o `7zz` pelo
   "diretório do executável"):

   ```bash
   sudo mkdir -p /opt/evezip
   sudo cp target/release/evezip vendor/7zz/linux-x64/7zz /opt/evezip/
   sudo chmod +x /opt/evezip/evezip /opt/evezip/7zz
   ```

2. Criar um link simbólico em `/usr/local/bin`:

   ```bash
   sudo ln -sf /opt/evezip/evezip /usr/local/bin/evezip
   ```

3. Instalar o atalho de menu/associação de MIME types copiando o
   `.desktop`:

   ```bash
   mkdir -p ~/.local/share/applications
   cp packaging/linux/evezip.desktop ~/.local/share/applications/
   update-desktop-database ~/.local/share/applications 2>/dev/null || true
   ```

## Windows

1. Copiar a pasta com `evezip.exe` e `7z.exe`/`7z.dll` (vendorizados em
   `vendor/7zz/windows-x64/`) para `C:\Program Files\EveZip\` — o `7z.exe`
   precisa ficar ao lado do `evezip.exe` para o motor localizá-lo pelo
   "diretório do executável".

2. Importar `packaging/windows/associar.reg` (duplo-clique ou
   `reg import associar.reg`) para associar `.zip`, `.7z`, `.rar`, `.tar`
   e `.gz` ao EveZip. Se a instalação não for em
   `C:\Program Files\EveZip\evezip.exe`, ajustar o caminho no `.reg` antes
   de importar.

## Divergência consciente da spec

A spec original pedia AppImage para Linux. A v1 entrega tarball manual +
`.desktop` (suficiente para uso interno). AppImage via `linuxdeploy` fica
como melhoria futura, quando houver máquina Linux de referência disponível
para validar o empacotamento.
