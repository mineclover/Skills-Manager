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
