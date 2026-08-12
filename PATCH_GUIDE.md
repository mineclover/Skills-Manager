# Skills Manager upstream / local patch guide

이 저장소는 원본 `jiweiyeah/skills-manager`와 로컬 기능을 같은 실행 트리에서
검증하되, Git 레인과 patch package로 변경의 소유권을 분리합니다.

## 구조

| 영역 | 역할 | 변경 원칙 |
| --- | --- | --- |
| `upstream/main` | 원본 레포 기준선 | upstream 변경만 반영 |
| `patches/skills-manager-control-plane` | 로컬 추가 기능의 patch stack | 새 로컬 기능은 이 레인에 먼저 커밋 |
| `main` | 실제 빌드·배포 대상 | 검증된 upstream + patch 결과만 반영 |
| `packages/skills-manager-patches` | patch manifest와 series | 기준 커밋과 보존 영역을 갱신 |

앱 소스를 `upstream/` 아래에 복제하지 않는 이유는 Tauri의 Rust workspace,
Vite 경로, frontend/backend 테스트 경로를 중복시키지 않고 upstream 병합 시
충돌을 한 곳에서 검토하기 위해서입니다.

## 최초 설정

```powershell
git remote add upstream https://github.com/jiweiyeah/skills-manager.git
git fetch --no-tags upstream main
git branch patches/skills-manager-control-plane backup/main-before-upstream-20260726
```

이미 remote 또는 branch가 있으면 `add`와 `branch` 명령은 다시 실행하지 않습니다.

## upstream 업데이트 수신

작업 트리가 clean이면 상태 확인과 fetch는 helper로도 실행할 수 있습니다.

```powershell
powershell -NoProfile -File tools/check-upstream.ps1
```

통합 브랜치 준비까지 자동화하려면 `-PrepareIntegration`을 사용합니다. 이 옵션은
merge를 시작하고 충돌 해결을 위해 멈추며, 파일을 자동으로 선택하거나 커밋하지 않습니다.

```powershell
powershell -NoProfile -File tools/check-upstream.ps1 -PrepareIntegration
```

1. 작업 트리를 깨끗하게 만들고 현재 통합 커밋을 기록합니다.

   ```powershell
   git status --short --branch
   git log -1 --oneline main
   ```

2. 원본을 fetch합니다.

   ```powershell
   git fetch --no-tags upstream main
   ```

3. upstream과 patch stack의 변경량을 먼저 검토합니다.

   ```powershell
   git log --oneline --left-right main...upstream/main
   git diff --stat main...upstream/main
   git diff --stat 186f8db7e62246062af5f836303a230652682a30..patches/skills-manager-control-plane
   ```

4. 통합 작업용 브랜치를 만듭니다.

   ```powershell
   $date = Get-Date -Format yyyyMMdd
   git switch -c integrate/upstream-$date main
   git merge --no-commit --no-ff upstream/main
   ```

5. 충돌을 해결할 때는 upstream 동작을 우선 검토하되 아래 로컬 제어면은 보존합니다.

   - `global`, `project`, `tool` scope와 scope별 instance ID
   - `src-tauri/src/services/scanner.rs`의 직접 설치된 CLI skill 탐지
   - `src-tauri/src/services/skill_control.rs`의 provider-aware 토글 경계
   - `.disabled-by-sm` 상태와 manager 비활성화 후에도 보이는 직접 skill
   - `Skills`/`Tools` 화면의 provider 선택, operation report, tool skill 토글
   - canonical `Skill` artifact를 enable/disable 때 변경하지 않는 규칙

6. 통합 브랜치에서 검증합니다.

   ```powershell
   npm test
   npm run build
   cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
   cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
   npm run inspect -- providers -- --json
   npm run inspect -- bindings -- --json
   git diff --check
   ```

7. 검증이 끝나면 통합 커밋을 만들고 `main`에 fast-forward 또는 일반 merge로 반영합니다.

   ```powershell
   git commit -m "merge: integrate upstream updates with CLI skill control"
   git switch main
   git merge --ff-only integrate/upstream-$date
   git push origin main
   ```

## 로컬 기능 추가

원본 커밋을 직접 `main`에 작성하지 말고 patch 레인에서 작업합니다.

```powershell
git switch patches/skills-manager-control-plane
git switch -c feat/my-local-change
# 구현 및 테스트
git commit -m "feat: describe the local change"
git switch patches/skills-manager-control-plane
git merge --ff-only feat/my-local-change
```

새 patch 커밋을 package에 반영하려면 다음을 실행합니다.

```powershell
git format-patch --no-signature --keep-subject `
  --output-directory packages/skills-manager-patches/patches `
  186f8db7e62246062af5f836303a230652682a30..patches/skills-manager-control-plane
```

그 후 `manifest.json`의 `baseCommit`, `lastReviewedCommit`, `integrationCommit`과
커밋 목록을 갱신합니다.

## 충돌 및 롤백 규칙

- upstream 병합은 `--no-commit`으로 멈춘 뒤 파일 단위로 검토합니다.
- `scanner.rs`, `skill_control.rs`, provider models/commands, `Skills.tsx`,
  `Tools.tsx` 충돌은 단순히 한쪽을 선택하지 말고 tool-scope fixture와 함께 확인합니다.
- 실패한 통합은 `git merge --abort`로 되돌립니다. 기존 `main`은 수정하지 않습니다.
- 통합 전 백업 브랜치를 남깁니다: `backup/main-before-upstream-<date>`.
- force push는 사용하지 않습니다.

## 완료 조건

upstream 동기화는 다음을 모두 만족해야 합니다.

- frontend test/build와 Rust test/fmt 통과
- `inspect providers`와 `inspect bindings`에서 `scope: "tool"` 항목 확인
- 직접 CLI skill, `.disabled-by-sm`, manager 비활성화 상태가 사라지지 않음
- 변경 범위와 충돌 해결 내용을 통합 커밋 메시지/PR에 기록
- `main`과 `origin/main`이 동일하고 작업 트리가 clean
