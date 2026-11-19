# M2WGame

[English](../../README.md) · [Tiếng Việt](README.vi.md) · [简体中文](README.zh-CN.md) · [Español](README.es.md) · [日本語](README.ja.md) · [한국어](README.ko.md) · **Português (BR)** · [Français](README.fr.md) · [Deutsch](README.de.md)

M2W = Mac para Windows (jogos). Um app para **rodar jogos de Windows em Macs com Apple Silicon**: gerencia "garrafas" do Wine (ambientes Windows independentes), motores Wine, o Steam para Windows e sua biblioteca de jogos, e explica os problemas em linguagem simples em vez de despejar uma montanha de logs.

> **Status: alfa inicial.** Funciona com alguns jogos e com outros não. Leia as "Limitações" antes de gastar tempo com um jogo. A interface do app está disponível em **vietnamita e inglês** (troque na barra lateral); outros idiomas são bem-vindos. O README em inglês é a referência e as traduções podem ficar defasadas.

> **Escopo:** o M2WGame foca em um único problema: rodar **jogos de Windows comprados na Steam**. Outras lojas e launchers (Epic, GOG, EA, Ubisoft, Battle.net, itch.io…) ainda não são suportados. Você pode adicionar um `.exe` avulso manualmente, mas esse caminho não foi testado.

O M2WGame **não é afiliado** à Apple, à Valve, à CodeWeavers nem ao projeto Wine. Ele não inclui, não baixa e não contorna nenhum jogo, DRM ou verificação de licença: você precisa ser dono dos seus jogos.

## O que faz

- **Biblioteca de jogos**: Jogar / Parar, configurações por jogo, logs e dicas em linguagem simples sobre o que deu errado.
- **Ambientes e motores**: crie ambientes, escolha o motor Wine de cada um e instale o Steam para Windows com um clique.
- **Steam sem a janela do Steam** (ela fica preta no Wine do macOS): adicione com um clique jogos que já estão na biblioteca do Steam; baixe jogos novos com o SteamCMD dentro do app (a senha e o código do Steam Guard vão direto ao SteamCMD e nunca são guardados); mostre dentro do app os diálogos ocultos do Steam (EULA, sincronização na nuvem) para você mesmo responder; avise sobre jogos de 32 bits; desinstale jogos para liberar espaço em disco.
- **Verificação do sistema e da rede**: Rosetta, versão do macOS, RAM, espaço em disco e se os servidores do Steam estão acessíveis.

## Requisitos

Mac com Apple Silicon, macOS 14 ou superior, Rosetta 2, Xcode Command Line Tools e [Rust](https://rustup.rs). Você precisa fornecer seu próprio motor Wine (não incluído); veja "Get an engine" no README em inglês.

## Download

Baixe o `.dmg` em [Releases](https://github.com/ryanle1203/m2wgame/releases) e arraste para Aplicativos. O app não é notarizado pela Apple: na primeira vez, clique com o botão direito > Abrir (detalhes na seção “Install” do README em inglês).

## Compilar

```bash
git clone https://github.com/ryanle1203/m2wgame
cd m2wgame
./scripts/build-swift.sh
open target/M2WGame.app
```

## Limitações

- **Jogos de Windows de 32 bits geralmente não rodam** (fecham ao iniciar); o M2WGame avisa.
- A interface do Steam não é utilizável no Wine do macOS; o projeto a contorna como descrito acima.
- Jogos com DirectX 12 ou antitrapaça ficam, em geral, fora de alcance.
- Testado apenas no macOS 27 com um M1 Pro; relatos de outros Macs são muito bem-vindos.

## Contribuir e licença

Leia o [CONTRIBUTING.md](../../CONTRIBUTING.md) (com regras firmes sobre senhas e DRM) e o [SECURITY.md](../../SECURITY.md). Licença: [MIT](../../LICENSE).
