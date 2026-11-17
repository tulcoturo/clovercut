# Upstream Review: Concat 0ef1e125..a7cda105 (v0.2.11+)

**Date:** 2026-11-17  
**Audited range:** `0ef1e12521f5727b9c9703c39bc0f607831fbb9c` .. `a7cda105c55cf4a5413dd747e16b0980ff2555c1`  
**Total commits:** 31  
**Upstream baseline:** Concat tag `v0.2.11` and following maintenance commits up to `upstream/main`.  
**Clovercut baseline:** Clovercut (`feat/capcut-draft-import` / `main`).

---

## 1. Resumo Executivo

Desde o último acompanhamento (`0ef1e125`), o upstream Concat lançou a versão `v0.2.11` e introduziu 31 commits. Como o **Clovercut já se distanciou substancialmente da arquitetura do Concat** (sistema próprio de legendas separadas de texto, efeitos de IA visual local, decodificador nativo de rascunhos CapCut, templates com slots, edição em 3 pontos e slip editing), grande parte dos commits upstream foi filtrada.

Identificámos **4 correções técnicas de alto impacto** que beneficiam diretamente a robustez e qualidade do Clovercut:

1. **Correção de Precisão no Arraste de Múltiplos Clipes (`40f13a6a`):**
   - No Concat/Clovercut anterior, o arraste de um grupo de clipes contíguos convertia os tempos para `f32` antes da soma. Ao final de timelines longas, o erro de arredondamento causava micro-sobreposições (~2e-5 s > `TOUCH`), fazendo com que o motor recusasse silenciosamente a movimentação.
   - Solução: função `shifted_starts` com aritmética pura em `f64` em [`placement.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-project/src/placement.rs).
2. **Correção de Localização com Vírgula Decimal em Filtros FFmpeg (`577e8532`):**
   - O FFmpeg lê parâmetros de filtros (ex: `atempo=1.5`) usando `strtod`, que respeita `LC_NUMERIC`. Em sistemas configurados em Português (pt-PT / pt-BR) ou outros idiomas europeus que usam vírgula como separador decimal, `atempo=1.5` falhava com `"Invalid argument"`, quebrando a velocidade de reprodução de áudio.
   - Solução: salvaguarda de `LC_NUMERIC` em [`concat-media`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-media/src/audio.rs) antes de invocar grafos de filtros.
3. **Resiliência do Detetor de Voz (VAD) para Transcrição Whisper (`ff9f7e74`):**
   - O detetor de voz anterior descartava áudio quando a fala estava misturada com música ou ruído de fundo, perdendo até 90% do conteúdo a transcrever.
   - Solução: o VAD mede a energia do sinal descartado; se descartar som tão audível quanto a voz, desativa o corte e entrega o áudio completo ao Whisper.
4. **Renderização de Títulos na Resolução de Exportação & Inicialização Assíncrona (`8b497918`):**
   - Títulos e legendas passam a ser rasterizados na resolução real do ficheiro de exportação (ex: 4K UHD) em vez da resolução de pré-visualização, eliminando legendas desfocadas ou pixelizadas na renderização final.

---

## 2. Inventário Completo de Commits e Decisões de Triagem

| Commit | Categoria | Sumário Upstream | Decisão Clovercut & Estratégia de Adaptação |
| :--- | :--- | :--- | :--- |
| `a7cda105` | CI / Workflows | Disparo manual de compilação de bundles desktop sem necessidade de tag | **Adaptar**: Permite testar empacotamento completo em CI sob demanda via `workflow_dispatch`. |
| `9d26aac8` | CI / Smoke | Smoke tests de exportação em Linux, Windows e macOS | **Adotar**: Validação automatizada de exportação em CI. |
| `218247fc` | CI / Android | Remoção de line tables em builds Android para poupar RAM do runner | **Adotar para Android**: Previne OOM durante a compilação do APK. |
| `76585480` | CLI / Smoke | Flags `--smoke-export` para testar exportação não-interativa | **Adotar**: Muito útil para testes headless e scripts de benchmark. |
| `552517cb` | Media / Android | Ajuste de parâmetros de cabeçalho do MediaCodec no primeiro pacote (#747, #786) | **Adotar para Android**: Evita travamento da renderização MP4 em Android 15 (API 35). |
| `3a10dc47` | CI / Android | Compilar tabelas de símbolos apenas no APK de release | **Adotar para Android**: Reduz tempo de compilação. |
| `6f04670d` | CI / Android | Ajuste de memória de swap para build Android | **Adotar para Android**: Estabilidade de CI. |
| `020d8cad` | CI / Dependencies | Bump de actions GitHub | **Adotar**: Manutenção preventiva de CI. |
| `517ac579` | i18n | Novas strings de aviso de projeto em 25 idiomas | **Adotar**: Mantém dicionários de tradução atualizados. |
| `f22453d8` | Android / Debug | Preservar símbolos da biblioteca como artefato para backtraces | **Adotar para Android**: Ajuda a diagnosticar crashes em dispositivos móveis. |
| `e72afe4c` | Logging | Alerta de última execução cita o erro real mais recente | **Adotar**: Maior clareza nos logs de recuperação. |
| `8b497918` | Host / Export | Títulos rasterizados na resolução de exportação e preparação assíncrona (#747) | **Adotar (ALTO IMPACTO)**: Elimina legendas borradas em exportações 4K e não bloqueia a UI ao iniciar renderização. |
| `ac843caa` | Android / UI | Início de exportação assíncrono em Android para não congelar janela | **Adotar para Android**: Mantém responsividade mobile. |
| `6d4b6710` | Branding | Ícones e banners do gato Concat em SVG e PNG | **NÃO APLICÁVEL**: O Clovercut possui identidade visual própria (trevo de 4 folhas). |
| `9d77f43b` | Logging | Log esclarece motivo de fallback para renderizador de software | **Adotar**: Transparência diagnóstica na GPU. |
| `942194dc` | Projeto / Core | Preservação de campos desconhecidos (`extra`) ao abrir projetos | **Adotar**: O Clovercut já tem suporte avançado a `extra: Map<String, Value>`. |
| `de9a6192` | CI / Governance | Políticas de fechamento de PRs no Concat | **NÃO APLICÁVEL**: Governança específica do upstream. |
| `ff9f7e74` | Speech / AI | VAD robusto: não descarta fala com volume similar ao ruído de fundo (#763) | **Adotar (ALTO IMPACTO)**: Melhora radicalmente a precisão do Whisper em clipes com música. |
| `577e8532` | Media / Áudio | Reseta `LC_NUMERIC` para evitar erro de vírgula decimal em filtros FFmpeg (#762) | **Adotar (CRÍTICO)**: Imprescindível para utilizadores em Portugal/Brasil/Europa. |
| `73ef32c1` | Docs / MCP | Documentação de ferramentas MCP | **Adotar**: Sincronização da documentação do servidor MCP. |
| `25e300e0` | Crash Reporter | Correção de falso positivo de crash em execuções do 0.2.8 | **NÃO APLICÁVEL**: Específico da árvore de migração do Concat. |
| `e6e85769` | GitHub | Formulário de bug report do Concat | **NÃO APLICÁVEL**: Infraestrutura do repositório upstream. |
| `40f13a6a` | Projeto / Timeline | Resolução precisa em `f64` ao arrastar grupos de clipes contíguos | **Adotar (ALTO IMPACTO)**: Corrige bug silencioso de colisão ao mover múltiplos clipes. |
| `b8d66e36` | UI / Slint | Correção de padding de primeiro caractere em caixas de redimensionamento | **Adotar**: Ajuste visual simples na interface Slint. |
| `6c9558aa` | UI / Monitor | Mensagem explícita na pré-visualização indicando arquivo em falta (#455) | **Adotar**: Usabilidade para relink de arquivos. |
| `30b33fc1` | UI / Notificações | Toast em cartão com suporte a multilinhas, cópia e botão de fechar (#455) | **Adaptar Seletivamente**: Adaptar a lógica de tempo e persistência de erros sem quebrar o layout já remediado do Clovercut. |
| `6584d349` | Release | Tag de versão 0.2.11 e READMEs de documentação | **NÃO APLICÁVEL**: Versionamento independente do Clovercut. |
| `de4f94bf` | i18n | 5 novas strings traduzidas | **Adotar**: Dicionários de idioma. |
| `798a57b1` | Style | Formatação de arquivos rustfmt | **Adotar**: Higiene de código. |
| `3f9798c6` | Testes / Texto | Teste de quebra de sequências de emojis no Windows | **Adotar**: Teste unitário. |
| `51699741` | Testes / Export | Leitura numérica de frames no teste de consolidação contra ruído de encoder (#756) | **Adotar**: Estabilidade da suíte de testes de consolidação. |

---

## 3. Plano de Ação Recomendado para o Clovercut

Podemos aplicar de imediato as correções cirúrgicas de maior valor sem nenhum risco de quebrar as inovações que já implementámos no Clovercut:

1. **Fase 1 (Motor & Timeline - `concat-project`):**
   - Integrar a função `shifted_starts` e precisão `f64` do commit `40f13a6a` em [`placement.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-project/src/placement.rs) e [`studio/gestures.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat/src/studio/gestures.rs).
2. **Fase 2 (Media & Áudio - `concat-media`):**
   - Integrar a salvaguarda de `LC_NUMERIC` do commit `577e8532` para blindar todos os filtros FFmpeg contra sistemas configurados em Português / Europa.
3. **Fase 3 (VAD / Whisper - `concat-speech`):**
   - Integrar a lógica de energia acústica do commit `ff9f7e74` em [`transcribe.rs`](file:///home/turotulco/Projetos/clovercut/src/crates/concat-speech/src/transcribe.rs) para evitar perda de palavras em músicas e diálogos suaves.
4. **Fase 4 (Exportação em Alta Resolução - `concat-host`):**
   - Garantir que a renderização de títulos use a resolução nativa da exportação (`8b497918`).
