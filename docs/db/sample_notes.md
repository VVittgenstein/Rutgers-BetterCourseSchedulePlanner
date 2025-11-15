# Spring 2026 SOC Sample Notes

## Snapshot summary
- 命令：`python3 scripts/fetch_soc_samples.py --year 2026 --term 1 --campuses NB,NK,CM --tag spring-2026`
- 抓取时间（UTC）：2025-11-15T20:25:26Z（参见 `data/raw/spring-2026-metadata.json`）
- term/campus 组合：Spring 2026 (`year=2026`,`term=1`) × New Brunswick (NB)、Newark (NK)、Camden (CM)。
- 所有响应均为 HTTP 200，`courses.json` 返回 `Cache-Control: max-age=900`，`openSections.json` 返回 `max-age=30`。

## Raw files
### courses.json（含 sections）
| Campus | Courses | Sections | Distinct Subjects | File | Size (MiB) | ETag |
| --- | --- | --- | --- | --- | --- | --- |
| NB | 4,608 | 11,680 | 238 | `data/raw/spring-2026-courses-nb.json` | 19.8 | `"166384036"` |
| NK | 1,210 | 2,397 | 86 | `data/raw/spring-2026-courses-nk.json` | 4.2 | `"1462284679"` |
| CM | 963 | 1,752 | 71 | `data/raw/spring-2026-courses-cm.json` | 3.2 | `"1538375825"` |

### openSections.json（section index 列表）
| Campus | Open section indexes | File | Size (KiB) | ETag | SHA256 |
| --- | --- | --- | --- | --- | --- |
| NB | 13,780 | `data/raw/spring-2026-openSections-nb.json` | 108 | `"592079634"` | `cb92ce87…6781` |
| NK | 13,780 | `data/raw/spring-2026-openSections-nk.json` | 108 | `"1130589031"` | `cb92ce87…6781` |
| CM | 13,780 | `data/raw/spring-2026-openSections-cm.json` | 108 | `"1759824602"` | `cb92ce87…6781` |

> 元数据（headers、payload 尺寸、SHA256、记录数等）集中在 `data/raw/spring-2026-metadata.json`，下游脚本可直接读取。

## Coverage observations
- `courseDescription` 字段在三个校区样本中全部为空；`synopsisUrl` 覆盖度差异较大（NB 约 74%、NK 21%、CM 18%），说明需要依赖 `synopsisUrl` 而非 description 以获得课程简介。
- `coreCodes`、`meetingTimes`、`sections[*].index` 等结构与 Fall 2024/Fall 2025 样本一致，所有课程均附带至少一个 section（`empty_sections = 0`）。
- `openSections.json` 在 NB/NK/CM 返回完全相同的 13,780 个 index（SHA256 一致），推测该端点在 Spring 2026 目前仍提供“全校区”开放席位列表，即使附带 `campus` 参数。因此映射时需依赖 `courses.json` 中的 `sections[*].index` 来过滤校区。
- `courses.json` gzip 体积 NB ≈0.85 MB、CM/NK ≈0.18 MB，在解压后分别为 20 MB / 4.3 MB / 3.2 MB，可作为规划存储/缓存策略的参考。

## Suggested usage
1. 解析 `data/raw/spring-2026-metadata.json` 动态获取文件路径、记录数和校区列表，减少硬编码。
2. 将 `openSections` 的“全校区”行为视作风险，在后续 schema 与轮询逻辑中保留 `campus` 过滤兜底，并监控 SHA 变化以检测官方行为变更。
3. 在字段分析阶段重点关注 `sections[*].meetingTimes`、`coreCodes`、`campusLocations`，以及缺失 `courseDescription` 带来的 UI 影响。
