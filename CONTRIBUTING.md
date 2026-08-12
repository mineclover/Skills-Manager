# 기여 가이드

Skills Manager는 upstream 원본과 로컬 patch를 분리해 관리합니다. 일반적인 개발
규칙은 [`DEVELOPMENT.md`](./DEVELOPMENT.md), upstream 동기화는
[`PATCH_GUIDE.md`](./PATCH_GUIDE.md)를 먼저 읽어주세요.

## 시작하기

```powershell
git clone https://github.com/mineclover/Skills-Manager.git
cd Skills-Manager
git remote add upstream https://github.com/jiweiyeah/skills-manager.git
npm install
cargo check --manifest-path src-tauri/Cargo.toml --bins
```

`upstream` remote가 이미 있으면 다시 추가하지 않습니다.

## 작업 유형 선택

- upstream 기능을 통합하는 작업: `integrate/upstream-YYYYMMDD`
- 이 저장소에만 필요한 기능: `patches/skills-manager-control-plane`에서 작업
- 일반 수정: `feat/*`, `fix/*`, `test/*`, `docs/*`, `refactor/*`, `chore/*`

로컬 patch 기능은 patch 레인에서 feature branch를 만든 후 완료된 커밋을 patch
레인에 fast-forward로 반영합니다.

```powershell
git switch patches/skills-manager-control-plane
git switch -c feat/my-local-change
# 구현 및 검증
git commit -m "feat: describe the local change"
git switch patches/skills-manager-control-plane
git merge --ff-only feat/my-local-change
```

통합 결과만 `main`에 반영하고, force push는 사용하지 않습니다.

## 개발과 검증

```powershell
npm run tauri dev
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

provider/scope를 변경했다면 다음도 확인합니다.

```powershell
npm run inspect -- providers -- --json
npm run inspect -- bindings -- --json
npm run inspect -- inspect -- --project <project-id> --json
```

테스트는 임시 HOME/project fixture를 사용합니다. 실제 사용자 skill root나 설정을
자동으로 enable/disable하지 않습니다.

## 커밋과 PR

커밋은 Conventional Commits 형식을 사용합니다.

```text
feat(control-plane): add project binding preview
fix(scanner): preserve disabled direct skills
test(scanner): cover skill discovery after add
docs(repo): update development conventions
```

PR에는 다음 내용을 포함합니다.

- 변경 목적과 upstream/local patch 분류
- provider와 scope 영향
- canonical skill과 shared root에 대한 영향
- 실행한 테스트 명령과 결과
- UI 변경 시 화면 캡처 또는 동작 설명

## 변경 시 지켜야 할 기준

- `Skill` canonical artifact와 `SkillBinding` provider 상태를 분리합니다.
- `global`, `project`, `tool` instance ID를 유지하고 legacy skill ID로 project/tool 대상을
  추론하지 않습니다.
- 직접 설치된 CLI skill, `.disabled-by-sm`, manager 비활성 provider의 skill도 숨기지 않습니다.
- Orca topic은 read-only inventory이며 filesystem skill처럼 토글하지 않습니다.
- 모든 mutation은 shared Rust service와 auditable operation report를 거칩니다.
- 문서 변경 시 관련된 README, 개발 규칙, patch guide, implementation plan을 함께 검토합니다.
