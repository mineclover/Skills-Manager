# Development conventions

이 문서는 현재 Skills Manager 저장소에서 기능을 구현·검토·릴리스할 때 적용하는
개발 규칙입니다. 제품 개요는 [`README.md`](./README.md), upstream 동기화는
[`PATCH_GUIDE.md`](./PATCH_GUIDE.md), control-plane 설계 기준은
[`IMPLEMENTATION_PLAN.md`](./IMPLEMENTATION_PLAN.md)을 참고합니다.

## 저장소 레인

이 저장소는 원본과 로컬 기능을 Git 레인으로 분리합니다.

| 레인 | 목적 | 허용되는 변경 |
| --- | --- | --- |
| `upstream/main` | `jiweiyeah/skills-manager` 원본 기준선 | fetch만 수행 |
| `patches/skills-manager-control-plane` | 로컬 추가 기능 patch stack | 로컬 기능, 회귀 테스트, 관련 문서 |
| `main` | 검증된 통합·배포 브랜치 | upstream 병합과 검증 결과 |
| `integrate/upstream-*` | 일회성 upstream 통합 작업 | 충돌 해결, 검증, 통합 커밋 |

새 로컬 기능은 `main`에 직접 작성하지 말고 patch 레인에서 개발한 뒤 통합합니다.
상세 절차는 [`PATCH_GUIDE.md`](./PATCH_GUIDE.md)의 upstream 업데이트 절차를 따릅니다.

## 아키텍처 규칙

- `Skill`은 canonical artifact이고 `SkillBinding`은 provider별 상태입니다.
- `global`, `project`, `tool` scope의 `instance_id`를 유지합니다. project/tool 작업에서
  legacy skill ID만으로 대상을 찾지 않습니다.
- enable/disable은 canonical skill 파일을 삭제·수정하지 않고 선택한 binding만 변경합니다.
- provider의 실제 root를 filesystem visibility의 기준으로 사용합니다.
- `~/.agents/skills`는 shared provider입니다. 영향받는 consumer를 확인하지 않은 상태에서
  직접 변경하지 않습니다.
- Orca topic은 로컬 설치 skill이 아닌 read-only runtime inventory입니다.
- 전역 scope가 기본 읽기 범위이며, project 읽기·쓰기는 항상 명시적인 project ID를 받습니다.

## 구현 순서

1. `src-tauri/src/services/`의 shared Rust service와 상태 전이를 먼저 구현합니다.
2. Tauri command와 `skills-manager-inspect` CLI가 같은 service를 호출하도록 연결합니다.
3. UI는 provider, scope, capability, operation report를 표시하고 service를 직접 우회하지 않습니다.
4. 새로운 상태 전이마다 임시 HOME/project fixture 기반 테스트를 추가합니다.
5. provider와 scope가 다른 상태를 하나의 boolean으로 축약하지 않습니다. `missing`,
   `conflict`, `unavailable`을 별도 상태로 유지합니다.

## 브랜치와 커밋

브랜치 이름은 다음 접두사를 사용합니다.

- `feat/`: 기능
- `fix/`: 버그 수정
- `test/`: 테스트·fixture
- `docs/`: 문서
- `refactor/`: 구조 개선
- `chore/`: 빌드·도구·의존성
- `integrate/upstream-YYYYMMDD`: upstream 통합

커밋 메시지는 Conventional Commits 형식을 따릅니다.

```text
<type>(optional-scope): short imperative description
```

예시:

```text
feat(control-plane): add project-scoped binding preview
fix(scanner): keep disabled direct skills actionable
test(scanner): cover project skill discovery after add
docs(repo): update upstream patch workflow
```

한 커밋에는 하나의 논리적 변경만 담고, 생성물(`dist`, `node_modules`, `target`)은
커밋하지 않습니다. `main`과 upstream 관련 브랜치에는 force push를 사용하지 않습니다.

## 코드 스타일

- TypeScript/React: 기존 2-space 스타일, 함수형 컴포넌트, 명시적인 provider/scope 변수명.
- Rust: `cargo fmt` 결과를 사용하고 filesystem mutation은 shared service 경계 안에 둡니다.
- UI/CLI에서 동일한 동작을 따로 구현하지 않습니다. 공통 report와 error state를 재사용합니다.
- 주석은 구현을 반복하기보다 scope/provider 선택 이유와 안전 제약을 설명합니다.
- 문서의 명령은 Windows PowerShell과 POSIX shell에서 차이가 있으면 환경을 표시합니다.

## Skill set assignment 역할과 저장소 호환성

`skill-sets.json` schema v3는 Catalog와 같은 역할 이름과 여러 작업 태그의 AND 조건을 사용합니다.

| 역할 | Effective set / activation 의미 |
| --- | --- |
| `default` | 활성 assignment이면 모든 작업 범위에 포함됩니다. |
| `work_scope_overlay` | 활성 assignment이고 모든 `work_scope_tags`가 요청 컨텍스트에 있으면 포함됩니다. 명시적인 빈 배열은 모든 범위에 적용됩니다. |
| `recommended` | 추천 후보로만 보관합니다. 범위가 일치하거나 `active: true`여도 effective set과 activation 대상에 포함되지 않습니다. |

후보를 적용하려면 같은 release를 `default` 또는 `work_scope_overlay`로 새로 assign합니다.
Manager의 assignment와 effective-set 요청은 `work_scope_tags: ["api", "review"]`를
지원합니다. 앞뒤 공백과 중복·빈 태그를 제거하며 대소문자는 유지합니다. 두 태그가 모두
있는 컨텍스트에만 overlay가 적용되고, 추가 컨텍스트 태그는 허용됩니다. UI에서는 쉼표로
태그를 입력합니다. 역할과 태그 조건 통일이 두 resolver의 revision 병합 방식까지 동일하다는
뜻은 아닙니다.

기존 `work_scope: "api"` 요청과 저장된 scalar는 하나의 태그로 계속 읽습니다. scalar에
쉼표가 있더라도 API와 저장소에서 여러 태그로 나누지 않습니다. 명시적인 배열이 있으면
scalar보다 우선하며 `work_scope`는 표시·이력용 문자열로 유지됩니다. 빈 배열 `[]`는
Catalog처럼 모든 범위와 일치합니다. effective-set 요청의 `[]`는 default와 모든 범위
overlay만 선택합니다. 배열이 없는 예전 빈 scalar overlay는 전체 범위로 확대하지 않고
매칭과 activation에서 제외합니다. 새 overlay와 effective-set 요청도 배열 없이 빈 scalar만 보내면
이전과 같이 오류를 반환합니다.

v0/v1에서 `recommended`는 작업 범위 overlay를 의미했습니다. 조회 시 해당 역할
(생략되어 기본값으로 읽힌 역할 포함)을 `work_scope_overlay`로 메모리에서 변환하고,
assignment ID, active 상태, priority와 scope를 보존합니다. 조회 자체는 저장 파일을
변경하지 않습니다. v2도 기존 scalar와 역할을 보존하여 메모리에서 v3로 읽습니다.
다음 정상 변경 저장에서 schema v3와 변환된 역할을 기록합니다. v3는 구버전 Manager가
태그 배열을 무시하고 표시 문자열을 단일 조건으로 오해하지 않도록 읽기를 거부하게 합니다.
구버전으로 되돌릴 때는 변경 전 저장소 백업이 필요합니다. v2/v3의 `recommended`를
legacy overlay로 재해석하지 않습니다.

API 클라이언트는 작업 범위 overlay를 만들 때 `role: "work_scope_overlay"`를 명시해야
합니다. 새 요청에서 역할을 생략하면 `recommended` 후보로 저장됩니다. Preview와 apply는
추천 후보를 거부하며 provider binding을 변경하지 않습니다. 미래 schema 버전은 읽기 오류로
종료하고 파일을 다시 쓰지 않습니다.

Codex의 repository binding은 skill 전달 루트 `.agents/skills`와 설정 루트 `.codex`를
분리합니다. Preview, scanner와 activation은 동일한 project skill 루트를 사용하며
`config.toml`은 `.codex`에 유지합니다. 기존 `.codex/skills`가 있더라도 새 프로젝트 전달은
`.agents/skills`를 사용합니다. 저장소 root가 없는 legacy binding과 전역 `ToolConfig` 경로는
기존 설정을 계속 사용합니다.

공유 영향 preview는 해당 skill의 scope와 project에서 계산한 실제 전달 루트를 기준으로
동일 디렉토리를 사용하는 consumer를 찾습니다. 프로젝트 `.agents/skills`는 해당 프로젝트의
경로로 표시합니다. 다른 root는 실제 공유 디렉토리 또는 source 의존성이 확인된 경우에만
포함하며 같은 도구라도 다른 root는 별도 영향으로 표시합니다. 이미 요청한 상태인 작업은 경로 정보를 유지하지만 확인을 요구하지
않습니다. 공유 source에서 독립된 target으로 링크만 추가하거나 Tool scope Codex 설정만
변경하는 경우도 공유 파일 변경으로 분류하지 않습니다.

Preview의 선택적 `target_root`는 요청 provider의 직접 skill binding 루트입니다.
계획 대상 비교는 이 필드를 사용하고, `impacts`는 같은 provider의 다른 root까지 포함할 수
있는 영향 목록으로 취급합니다. 필드가 없는 legacy 응답은 첫 번째 요청 provider impact를
직접 대상으로 해석할 수 있습니다. 새 응답에서도 요청 provider의 직접 대상은 첫 항목으로
유지합니다.

직접 설치된 source 디렉토리의 rename/복구는 설정된 전역·프로젝트 root의 해당 skill
binding만 추가로 조사합니다. source를 경유하는 간접 symlink consumer는 자신의 root와
의존 이유를 표시하며, 복구 대상의 dangling link는 `currently broken`으로 설명합니다.
symlink chain은 최대 64 hop으로 제한하고 cycle이나 해석 불가 경로는 mutation 전에
실패합니다. 전체 홈 디렉토리 탐색은 하지 않으며, source alias를 거치지 않고 같은 실제
대상을 직접 가리키는 독립 링크는 영향에 포함하지 않습니다.

단일 토글, 일괄/그룹, preset, skill-set 적용은 실제 공유 변경이 있으면 mutation 전에
preview를 확인해야 합니다. Tauri mutation의 `confirmShared`는 기본 false이며 batch와
preset도 전체 작업을 먼저 검사하여 미확인 공유 작업이 있으면 변경 전에 중단합니다.
Inspector의 skill enable/disable, batch, preset apply에는 `--confirm-shared`를 전달합니다.
확인은 해당 요청에서 검토한 범위에만 적용되며 전역 설정이나 영구 승인을 저장하지 않습니다.

## 검증 명령

기본 PR 검증:

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
git diff --check
```

control-plane 변경은 다음 read-only smoke check도 실행합니다. 상태를 변경하는 CLI
명령은 shared root 영향과 `--confirm-shared` 요구 사항을 별도로 검토합니다.

```powershell
npm run inspect -- providers -- --json
npm run inspect -- bindings -- --json
npm run inspect -- -- --help
```

project/provider 변경은 명시적인 project ID로 다시 확인합니다.

```powershell
npm run inspect -- inspect -- --project <project-id> --json
npm run inspect -- providers -- --project <project-id> --json
npm run inspect -- bindings -- --project <project-id> --json
```

테스트는 실제 사용자의 `~/.skills-manager`, `.claude`, `.codex`를 변경하지 않고
임시 HOME, 임시 project root, fixture를 사용합니다. 실제 CLI mutation을 수동으로
검증해야 할 때는 대상과 원상복구 절차를 먼저 기록합니다.

## PR 체크리스트

- [ ] 변경이 upstream 코드인지 local patch인지 분류했다.
- [ ] provider와 scope가 모든 mutation 경로에 명시되어 있다.
- [ ] canonical artifact가 enable/disable 중 변경되지 않는다.
- [ ] 직접 설치된 CLI skill과 `.disabled-by-sm` 상태가 계속 보인다.
- [ ] project skill 추가 후 재스캔·instance ID 인식 테스트가 있다.
- [ ] operation report의 applied/skipped/failed/impact를 확인했다.
- [ ] frontend test/build와 Rust fmt/test를 실행했다.
- [ ] README, `CONTRIBUTING.md`, `PATCH_GUIDE.md`, `IMPLEMENTATION_PLAN.md` 중
      영향을 받는 문서를 갱신했다.
