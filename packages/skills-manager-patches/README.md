# Skills Manager local patch package

이 패키지는 `jiweiyeah/skills-manager` 원본에 추가한 로컬 기능을 추적하기 위한
manifest와 재현 가능한 patch series를 보관합니다. 실행 가능한 Tauri 앱은 기존
저장소 루트에 유지하고, 이 패키지는 upstream 소스의 복사본을 중복 보관하지 않습니다.

## 레인

- `upstream/main`: 원본 저장소의 최신 기준선
- `patches/skills-manager-control-plane`: 로컬 기능만 담은 선형 patch stack
- `main`: upstream과 patch stack을 검증한 통합 빌드
- `manifest.json`: 기준 커밋, patch 커밋, 보존해야 할 제어면을 기록
- `patches/*.patch`: 원본 기준선에 적용할 수 있는 patch series

세 레인은 역할이 다릅니다. upstream 기능을 로컬 patch 커밋으로 섞지 않고, 통합
결과만 `main`에서 릴리스합니다.

동기화 절차는 저장소 루트의 [`PATCH_GUIDE.md`](../../PATCH_GUIDE.md)를 따르세요.
