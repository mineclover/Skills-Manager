import { lazy, Suspense, useState, useEffect, useCallback } from "react";
import { BrowserRouter, Routes, Route, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { Layout } from "@/components/layout/Layout";
import { Welcome } from "@/pages/Welcome";
import { useInitialization } from "@/hooks/useInitialization";
import { ThemeProvider } from "@/hooks/useTheme";
import { SkillTranslationProvider } from "@/hooks/useSkillTranslation";
import { AuthProvider } from "@/contexts/AuthContext";
import { I18nProvider, Language } from "@/i18n";
import { FontFamilyPreset, normalizeFontFamilyPreset } from "@/lib/fontFamily";
import { AppConfig, MarketplaceUpdateCheckResult } from "@/types";
import { ToastContainer, useToast } from "@/components/ui/toast";
import { CommandPalette } from "@/components/CommandPalette";
import { PageHeaderProvider } from "@/components/PageHeaderContext";

type Theme = "light" | "dark" | "system";

const Skills = lazy(() => import("@/pages/Skills").then(({ Skills }) => ({ default: Skills })));
const Tools = lazy(() => import("@/pages/Tools").then(({ Tools }) => ({ default: Tools })));
const Presets = lazy(() => import("@/pages/Presets").then(({ Presets }) => ({ default: Presets })));
const SkillSets = lazy(() => import("@/pages/SkillSets").then(({ SkillSets }) => ({ default: SkillSets })));
const ProjectProfile = lazy(() => import("@/pages/ProjectProfile").then(({ ProjectProfile }) => ({ default: ProjectProfile })));
const ReviewQueue = lazy(() => import("@/pages/ReviewQueue").then(({ ReviewQueue }) => ({ default: ReviewQueue })));
const Marketplace = lazy(() => import("@/pages/Marketplace").then(({ Marketplace }) => ({ default: Marketplace })));
const Settings = lazy(() => import("@/pages/Settings").then(({ Settings }) => ({ default: Settings })));
const Feedback = lazy(() => import("@/pages/Feedback").then(({ Feedback }) => ({ default: Feedback })));
const EditorPage = lazy(() => import("@/pages/Editor").then(({ EditorPage }) => ({ default: EditorPage })));

function GlobalShortcuts({ onOpenPalette }: { onOpenPalette: () => void }) {
  const navigate = useNavigate();
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const isMod = e.metaKey || e.ctrlKey;
      if (!isMod) return;
      // Cmd+K / Ctrl+K: open command palette
      if (e.key === "k" || e.key === "K") {
        e.preventDefault();
        onOpenPalette();
        return;
      }
      // Cmd+, / Ctrl, (macOS Settings convention): open settings
      if (e.key === ",") {
        e.preventDefault();
        navigate("/settings");
        return;
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [navigate, onOpenPalette]);
  return null;
}

function App() {
  const { isInitialized, isLoading: initLoading, markInitialized } = useInitialization();
  const [language, setLanguage] = useState<Language>("en");
  const [theme, setTheme] = useState<Theme>("system");
  const [fontFamily, setFontFamily] = useState<FontFamilyPreset>("default");
  const [configLoaded, setConfigLoaded] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const { toasts, removeToast } = useToast();

  // Load preferences from config on mount
  useEffect(() => {
    async function loadPreferences() {
      try {
        const config = await invoke<AppConfig>("get_config");
        if (config.preferences?.language) {
          setLanguage(config.preferences.language as Language);
        }
        if (config.preferences?.theme) {
          setTheme(config.preferences.theme as Theme);
        }
        setFontFamily(normalizeFontFamilyPreset(config.preferences?.font_family));
        } catch {
        // Use defaults on error
      }
      setConfigLoaded(true);
    }
    loadPreferences();
  }, []);

  const handleLanguageChange = useCallback((lang: Language) => {
    setLanguage(lang);
  }, []);

  const handleThemeChange = useCallback((newTheme: Theme) => {
    setTheme(newTheme);
  }, []);

  const handleFontFamilyChange = useCallback((newFontFamily: FontFamilyPreset) => {
    setFontFamily(newFontFamily);
  }, []);

  useEffect(() => {
    if (!isInitialized || !configLoaded) {
      return;
    }

    const timer = window.setTimeout(() => {
      void invoke<MarketplaceUpdateCheckResult>("check_marketplace_updates_if_stale").catch(
        () => {
          // keep startup check silent on failures
        },
      );
    }, 20_000);

    return () => {
      window.clearTimeout(timer);
    };
  }, [configLoaded, isInitialized]);


  // Wait for both initialization check and config to load
  if (initLoading || !configLoaded) {
    return (
      <div className="min-h-screen bg-background flex items-center justify-center">
        <div className="animate-pulse text-muted-foreground">Loading...</div>
      </div>
    );
  }

  if (!isInitialized) {
    return (
      <ThemeProvider
        theme={theme}
        fontFamily={fontFamily}
        onThemeChange={handleThemeChange}
        onFontFamilyChange={handleFontFamilyChange}
      >
        <I18nProvider language={language} onLanguageChange={handleLanguageChange}>
          <Welcome onComplete={markInitialized} />
        </I18nProvider>
      </ThemeProvider>
    );
  }

  return (
    <ThemeProvider
      theme={theme}
      fontFamily={fontFamily}
      onThemeChange={handleThemeChange}
      onFontFamilyChange={handleFontFamilyChange}
    >
      <I18nProvider language={language} onLanguageChange={handleLanguageChange}>
        <AuthProvider>
          <BrowserRouter>
            <PageHeaderProvider>
              <SkillTranslationProvider>
                <GlobalShortcuts onOpenPalette={() => setPaletteOpen(true)} />
                <Suspense fallback={<div className="min-h-screen bg-background flex items-center justify-center"><div className="animate-pulse text-muted-foreground">Loading...</div></div>}>
                  <Routes>
                    <Route path="/" element={<Layout onOpenPalette={() => setPaletteOpen(true)} />}>
                      <Route index element={<Skills />} />
                      <Route path="tools" element={<Tools />} />
                      <Route path="presets" element={<Presets />} />
                      <Route path="skill-sets" element={<SkillSets />} />
                      <Route path="projects" element={<ProjectProfile />} />
                      <Route path="review-queue" element={<ReviewQueue />} />
                      <Route path="marketplace" element={<Marketplace />} />
                      <Route path="settings" element={<Settings />} />
                      <Route path="feedback" element={<Feedback />} />
                    </Route>
                    <Route path="/editor" element={<EditorPage />} />
                  </Routes>
                </Suspense>
                <CommandPalette open={paletteOpen} onClose={() => setPaletteOpen(false)} />
                <ToastContainer toasts={toasts} onRemove={removeToast} />
              </SkillTranslationProvider>
            </PageHeaderProvider>
          </BrowserRouter>
        </AuthProvider>
      </I18nProvider>
    </ThemeProvider>
  );
}

export default App;
