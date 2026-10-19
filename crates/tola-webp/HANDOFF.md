# Tola WebP 编码器交接 · 2026-10-18

## 先读结论

当前工作目录 `/Users/kawayww/projects/tola-ssg-dev`。记录时 HEAD 为 `3c0dea99c5`；并发会话仍在提交，工作区有未提交的 Phase 1 多核、ChromaWinner 和证据文件。接续先重读 HEAD、状态和实际源码。不要回退到基线 `83c6925943`。

**24 点直接对比**：体积对 libwebp 22/24 点更小（geomean 0.94×；q50–q90 全胜），对 zenwebp 20/24 更小；峰值 RSS 13.3 MB vs libwebp 29.6 MB、zenwebp 73.9 MB；**同 effort 单线程**（m5/m6 trellis）三点均更快、PSNR 更高、匹配 PSNR 体积更小，详见 method-matched-comparison.json；速度（快照 vs libwebp **method 4，无 trellis**）1.31×、libwebp MT(2 worker) 1.39×，zenwebp 为 libwebp 1T 的 1.36×（我们略快于 zenwebp）。**默认已切 restricted**（`encode_rgba`；`encode_rgba_with_mode` 仍可选 Exact）：corpus 184,854（+16 B）、36 点 3,463,256、restricted 对 m5 0.884×/23-24、对 m4 0.998×。Phase 2 多核已集成（K=2/4 按面积与列数）；**最终 24 点**（安静主机）auto-K 时间 **0.470× libwebp 1T、0.488× MT(2)、0.351× zenwebp**，24/24 更快；体积 **0.934× libwebp（24/24 更小）、0.942× zenwebp**。单核 exact 矩阵 24 点：对 m4 1.185×、对 m5 1.049×、对 m6 **0.567×**（时间），体积 0.941/0.93-1.00/0.96-1.00。

pure profile 仍为 **10**：350 图当前 **184,838 字节**（gated 精确概率验收修复两个 +2 B 回归；此前 184,842），32×32 pin **134 字节 / FNV-1a `0xea01e20491288399`**。36 点质量矩阵：35 点通过 1.1× bracket，1 点在 cwebp 观测上限外；最坏 bracket **1.0448×**；全部 PSNR 与 profile 9 相同；总字节 3,544,128 vs 3,545,368。

速度检查点：同进程 pure/native **1.19× / 1.49× / 1.52×**（photo / gallery-3 / gallery-2，最干净一轮；主机负载下 native 同进程对照），历史为 1.52 / 1.74 / 1.67。整体目标仍未完成：没有干净主机的完整矩阵、没有当前 zenwebp 可执行对比、也没有证明速度超过 libwebp。

## 用户要求与工作方式

- 目标覆盖正确性、纯 Rust、确定性、SIMD、速度、压缩比、内存、质量；单核和多核都要推进；参考现代论文、理论、算法和数据结构。zenwebp 是只读算法参考，AGPL 代码不复制、不逐行翻译；BSD libwebp 可作为实现参考并署名。
- 明确要求并发子代理；常规选择自行完成；不扩展到其他会话的代码。唯一禁用的子代理模型是 `6 astra`；派发后核对实际路由。
- shell 命令以 `rtk` 开头；原始输出用 `rtk proxy`；仓库验证用 `--locked`。
- 核心保持 `#![forbid(unsafe_code)]`、整数决策、固定模式顺序和明确平局规则。运行时依赖只有 `fearless_simd` 和 `image-webp`；后者带入 `byteorder-lite`、`quick-error`。
- 注释只写代码无法表达的约束、原因或移除条件；不写流程旁白、不复述代码。局部变量和 helper 只在承载语义时引入；测试优先合并/拆分重复准备，不删除独立 oracle。

## 权威入口

| 需要了解的内容 | 入口 |
|---|---|
| 当前 API、算法边界、复现命令 | [README.md](README.md) |
| 当前字节的 36 点质量矩阵 | quality-revision10.json、rate-distortion-revision10.csv |
| 前三个 profile 10 变化的收敛记录 | convergence.json、convergence.csv |
| 现代论文、算法、数据结构与并行调研 | modern-optimization-survey.md |
| loop filter 强度实验（探索性，未集成） | filter-level-study.json |
| 已测负结果（trellis/lambda/gamma/bool/容器） | negative-results.json |
| zenwebp 算法与 SIMD/内存对照 | zenwebp-algorithms.md、zenwebp-simd-memory.md |
| 直接对比（libwebp 1T/2W、zenwebp 1T） | direct-comparison.md |
| 同 effort 单线程（libwebp m4/m5/m6） | method-matched-comparison.json |
| 最终 24 点单线程（m4/m5/m6） | single-thread-comparison.json |
| 最终 24 点多核（1T/MT、zenwebp） | direct-comparison.md |
| 最终审计 | final-audit.md |
| 独立解码/容器正确性审计 | correctness.md |
| 真实/合成图片压测（tola/cwebp/ImageMagick/ffmpeg） | stress.md |
| profile 9 历史质量与计时 | rate-distortion.csv、results.json、timing-api.csv、timing-structure.json |

研究文档固定到 zenwebp `c4b6614eba263c908e7317f6f8d8689fbbd338d3`；本地只读副本为 `/tmp/zenwebp`。BSD libwebp 在 `libwebp-sys 0.9.6/vendor/`。不要把不同版本的行为混入结论。

## 已落地的输出保持改动

- I16/chroma finalist 复用最近邻阶段的前向变换：每宏块省 16 个 luma DCT、8 个 chroma DCT 和 1 个 WHT；最大图同进程 A/B 约 -2.1%。
- `sample_block` 逐行 4 字节拷贝；同进程三点 A/B 约 -3%~-4%。
- `chroma_row` 每轮 16 个 chroma cell 的 SIMD 转换，奇宽与填充走标量；内核 3.3~3.5×，约 0.55~0.75 ms/帧。
- ALPH 直写 sink（省一次分配/拷贝，raw 回退提前中止）、word-wise opacity 扫描；VP8 mode leaf 预计算路径；raw alpha 峰值 RSS -7.4%。
- trellis 的 λ 缩放 token 率按 plane/position/context 预计算，增量 suffix 失真；`last_nonzero` 分组扫描。指令数 -7.7%~-10.5%。
- token path 表把 `(probability offset, branch)` 作为 const 步骤，emission/counting 共享；去掉 `block_rate` 的冗余零折叠。指令数 -1.9%~-5.7%。
- ALPH 严格边界：`finish` 只在压缩载荷严格小于 raw 时返回；`reconstruction::Plane` 增加 2 的幂断言。
- bool writer 快路径：`range >= 128` 直接返回、单次 shift 快路径、进位传播 `#[cold]`；字节不变，gallery-3/gallery-2/checker 指令 -0.3~-0.6%，photo 中性。
- 系数 spool：分析用 `DenseMacroblock`，存储改为 64 B 打包宏块 + 64 KiB chunked spool；token 直接读 scan 前缀。4096² q75 峰值 RSS -18.4%、2048² -19.4%，常规点指令 -0.7~-2.9%，dense noise +0.49% RSS。
- 多核 Phase 1：alpha 与主分析重叠、模式分区与残差分区并行；native 专用、wasm 串行；350 图与三个计时点字节不变。ChromaWinner 收束 chroma 选择状态。
- pair-loop subset：6-bit pair 表 + 重排 pair loop（packed scan offsets、outlined wide path、inline tail）、u32 trellis rates；q90/q95 指令 -1.97%、cycles -1.38%（photo cycles 中性，galleries 全改善）；字节不变、71 测试。
- histogram token counting：fit 不再逐 branch 遍历，TokenCounts 按 (entry, skipped-EOB, token) 直方图，每帧一次展开成 BranchCounts；counting sink 跳过固定 extra/sign 位，ENTRY_BASE + mask 索引。q90/q95 单核指令 geomean **-4.84%**（default release）/ **-3.53%**（thin-LTO），wall -5.87%/-4.61%；token-counting leaf 6.4% → 1.7%；字节完全一致。
- q95 归因 + 6-bit coefficient-pair 表 + `write_bool` inline：q95 的 block_rate 升到 19%、bool/token 发射 22%；pair6 把 g3 q95 指令 -2.2%、g3 q75 -0.28%、photo q95 -0.07%，字节完全不变；表 +0.74 MB。
- chroma 块级 SIMD：8×8 平面四个子块装 4 lane，kernel 2.0×，单核 photo -2.20%、g3 -1.43%、g2 -0.96% 指令；字节完全一致。
- mode-cost floor（exact）：`analyze_i4` 在预测/量化前用 `lambda*mode_cost/256 >= limit` 剪枝；2048² q75 -16.0%、photo -9.4%、g2 -0.4%、g3 +0.09%（噪声级）指令；字节/质量不变。SSE 下界可证明但 +14.1%，I16/chroma 扩展 0 命中且 +0.27~0.55%，均拒绝。
- speed 模式（opt-in）：`TrellisMode::Exact`（默认）与 `Restricted`；restricted 是 libwebp 式两候选 DP，36 点 +0.02% 字节、+0.0017 dB 均值、bracket 不变。1T vs libwebp m4：photo 0.996× 时间/0.873× 体积（双赢）；g3 1.254× 时间、g2 1.393× 时间（体积仍赢）。remaining gap 在 mode search/rate walks。
- 自适应 I16/I4 顺序：每图一次 luma activity 分类（阈值 1200）；平滑图 v4a 顺序，纹理图 I4-first + `min(N, I4+1)` 精确界；字节一致，单核指令 geomean -2.7%、wall -2.9%，held-out gallery-3/4/5 全赢、photo 中性。
- Phase 2 竖条一行斜置流水线：K=2/4 按面积与列数；native 并行、wasm 串行；350 图、2048²/4096²、K=1/2/4/8 字节一致。3 点 pure/native 0.38/0.46/0.77× vs 1T、0.39/0.51/0.75× vs MT(2)（4 线程）。
- gated 精确概率验收：仅当估计净收益 <=8 B 或面积 <128² 时做长度精确比较；350 图 184,842→**184,838**，两个 +2 B 回归修复，指令开销 <=0.06%。ChromaWinner 收束 chroma 选择状态。

## 证据范围

- 字节：350 图、pin；69 项 crate 测试 + 94 项 `tola-image` pure-webp = **163**；clippy/fmt。当前代码在 native、WASI 标量、WASI SIMD128、x86_64 Rosetta 下 350 图全部一致。
- 指令/内核：rate 率表 + 分组扫描 -7.7~-10.5%；token path -1.9~-5.7%；chroma SIMD 内核 3.3~3.5×；alpha/vp8 -0.3~-0.9%；`sample_block` 同进程 A/B -3~-4%；I16/UV 复用最大图 -2.1%；系数 spool 常规点指令 -0.7~-2.9%，峰值 RSS -18~-19%（4096²/2048² q75），dense noise 最坏 +0.49% RSS。
- 质量：36 点矩阵 35 pass + 1 ceiling inconclusive，最坏 1.0448×，PSNR 与 profile 9 相同。
- 24 点直接对比（快照）：体积 22/24 小于 libwebp（geomean 0.94×），20/24 小于 zenwebp；时间 1.31× libwebp 1T、1.39× libwebp MT(2 worker)，zenwebp 1.36× lib 1T；峰值 RSS 13.3 MB vs 29.6 MB libwebp、73.9 MB zenwebp。详见 direct-comparison.md。
- 计时：24 点直接对比（快照，1T）1.31× libwebp 1T、1.39× libwebp MT；Phase 2 K=4 后当前 3 点 0.43/0.65/0.81× native 1T（同进程 21 次×2 轮）；完整 24 点安静主机复跑待做。更早的脏矩阵（0.8~2.6）无效。
- `filter-level-study.json` 是探索性结果：当前 `quantizer/2` 在照片上接近最优；逐图选择的理论上限均值 +0.056 dB、最高 +0.55 dB。尚未实现选择器。

## 性能归因（最终 1T，photo q75，leaf samples）

| 开销 | 占比 |
|---|---:|
| `trellis::Quantizer::quantize` | 29.5% |
| `encoder::analyze_i4` | 23.2% |
| `token::TokenCosts::block_rate` | 17.0% |
| `encoder::analyze_luma` | 10.7% |
| `bool_writer::write_bool` | 4.6% |
| chroma/Borders/spool/WHT/misc | 15.0% |

exact trellis + block_rate = 46.5%；这是对 m4 18.5%、对 m5 4.9% 单核时间差的来源。下一步 exact 目标是 block_rate 的 17%。完整审计见 final-audit.md。

## 源码接续地图

| 入口 | 接续时必须守住的边界 |
|---|---|
| `encoder.rs` | 分析期仍用稠密 y/u/v；I16/UV finalist 复用前向变换；I4 先选候选再 trellis；固定模式顺序、严格 `<` 平局、所有权边界 |
| `macroblock.rs` | 分析用 `DenseMacroblock`；存储为 64 B 打包宏块 + chunked `CoefficientSpool`；Intra16 DC 仍留在 `Luma` 中 |
| `reconstruction.rs`、`predict.rs` | 两行重建环保留原始 above-right；`row_mask` 依赖 2 的幂行数 |
| `token.rs` | `TokenPath` 步骤同时服务 emission/counting；`last_nonzero` 分组扫描；计数与输出访问同一系数分支 |
| `trellis.rs` | 精确 3 上下文 DP；`PlaneRates` 预计算；受限 2 候选只作为实验，不是默认；不要把它当成等价优化 |
| `color.rs` | RGB 先扩边再平均；chroma SIMD 与标量方程、lane 顺序、截断一致 |
| `alpha.rs`、`container.rs` | ALPH 直写依赖 pinned `image-webp 0.2.4` 的简单容器布局；`finish` 校验头部/长度/严格小于 raw；码流与容器各自只拥有自己的头 |

## 未完成事项与接续次序

1. **单核速度**：自适应 I16/I4 顺序（进行中）；融合计数已实现且字节一致，但 photo 仅 -0.11% 指令、与 Phase 2 需手工 rebase，暂未集成。I16 精确早停、Borders、bool 快路径已落地。零尾/受限 trellis/lambda/SIMD DP/模式级 I4 SIMD 均已测负。
2. **多核**：Phase 1/2 已落地；Phase 2 为竖条一行斜置流水线，K=2/4 按面积与列数，wasm 串行。剩余：安静主机 24 点复跑、K=8/自动核数评估、把计数/拟合也并行化。验收记录：K=4 三点 0.43/0.65/0.81× native 1T，2048² 2.05× Phase2-only。
3. **内存**：系数 spool 已集成。后续只剩 Intra16 DC 打包（约 20~25 B/MB）和极小图的 64 KiB chunk 下限。
4. **压缩**：概率拟合的 gated 精确验收（unconditional 已测负）；lambda 归一化和两段方差 segmentation 已测负。若再做 segmentation，必须换 RD/DCT 直方图分类器并加 per-segment filter，用新的 matched-PSNR gate。
5. **质量**：loop filter 强度选择需要 normative filter 和速度预算；gamma 线性光已测负，若再做需 SharpYUV 风格迭代与显式 decoded-RGB 目标。
6. **最终审计**：安静主机 full 24 点性能矩阵、36 点质量复核、当前 x86/Rosetta 计时、真正的 zenwebp 可执行对比、完整 workspace gate、README/HANDOFF 最终更新。

## 不要重做的实验

- 固定模式两遍概率重放：匹配 release 下体积和速度都变差。
- 更弱全局 loop filter：训练样本改善、保留样本回退。
- bool writer 的一次性 bulk normalization：指令数无变化；转向分支化/进位路径。
- VP8 帧缓冲直写：字节一致但峰值 RSS 与指令无可测变化。
- 预采样源块的 hoist：同进程 A/B 落在噪声内。
- 更大的 trellis edge cache、`inline(always)` subblock、fused luma/chroma rate：已测无收益或回归。
- 受限 trellis 若质量矩阵不过：不要以“更快”为由替换精确 DP。
- 受限 2 候选 trellis：36 点 +542 B、均值 PSNR +0.0017 dB、速度 photo/g3 -5.8~-8.3% 但 g2 q95 +2.5%；已拒绝。
- lambda 归一化：×2 -3.67% 字节但 -0.260 dB；×4 -9.79% 但 -0.854 dB；匹配 PSNR 后无收益，已拒绝。
- gamma 线性光色度：字节 -0.100%，但 PSNR -0.1033 dB、SSIM -0.000322、5 点 bracket 退化；已拒绝。
- bool 进位延后：真实流 5.9~6.4 → 6.2~7.1 ns/write，已拒绝；改用快路径。
- 模式级 I4 SIMD：内核 1.32×，但集成后 +0.53%/+0.64% 指令；已拒绝。
- I16 块级 SIMD：每块已吃满 4-lane i32，块批处理 +0.5~0.8% 端到端指令；已拒绝。128-bit SIMD 在模式搜索上饱和。
- token buffer：事件缓冲 photo -1.1% 但 g2 +1.68%（事件密度）、level 缓冲四项全负；3~5% 目标未达，已拒绝。
- block_rate u32+inline：photo +0.06%、g3 +0.05% 指令，字节一致；已拒绝。17% 是 pair-table 查表成本。
- branchless write_bool：photo +0.25~0.58%、g3 +0.53~0.65% 指令，字节一致；已拒绝。
- 融合计数：字节一致、计数逐宏块证明正确，但 photo 仅 -0.11% 指令；收益边际，暂不集成。
- 两段方差 segmentation：-2.82% 字节但 -0.15 dB 均值，匹配 PSNR 后 +1.02% 体积，q95 bracket 1.1279；已拒绝。
- RD 分类 segmentation：量化分段本身 parity；收益全部来自全局 filter offset（q/2-4，大图），19 正/13 零/4 负、最差 -0.022 dB、均值 +0.018 dB、字节不变；segmentation 拒绝，filter offset 记为 free quality 选项但不默认。
- 4 段 k-means segmentation：-1.17% 字节但 -0.022 dB 均值，matched 0.99873；photo 1.012× 更差，4096² 文件 +0.61%，map ~1.7 bit/MB；拒绝。segmentation 线关闭。
- 以上负结果见 negative-results.json。

## 辅助入口

这些路径不作为唯一证据，可能被清理；仓库内文档优先。

- 子代理副本与实验：`/tmp/tola-webp-agents/*`；同进程双版本：`/tmp/tola-webp-paired-harness`；纯/native 计时：`/tmp/tola-webp-rgba-api-timing`。
- 质量来源与输出：`/tmp/tola-webp-final-B-evaluation`；cwebp sweep：`/tmp/tola-webp-chroma-coverage/measurements.json`。
- 可复用子代理：`/root/token_performance`、`/root/zen_simd_memory`、`/root/zen_algorithms`；接续先确认仍存在，再恢复或重新派发。

## Suggested skills

技能文件位于 `.agents/skills/<name>/SKILL.md`；按触发条件读取。

- `skill://subagent-delegation`：派发/恢复子代理前，确定独占文件并核对实际模型。
- `skill://naming`、`skill://comments`：修改 Rust 标识符或注释前。
- `skill://trim`：修改测试或清理重复逻辑前。
- `skill://refactoring`：承诺字节保持不变的结构改动。
- `skill://research`：查阅规范、上游实现或论文时，使用一手来源并保存可追溯结论。
- `skill://concurrency-and-failure`：改变线程、缓存、缓冲区、共享所有权或外部验证进程时。
- `skill://scope-discipline`：接触另一个子代理或并行会话的文件时。
- `skill://writing-for-agents`：更新本交接文档时，以现有证据为入口，减少重复叙述。
