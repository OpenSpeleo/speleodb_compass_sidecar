import { useUiState } from "./lib/state";
import { AuthScreen } from "./components/auth-screen";
import { MainLayout } from "./components/main-layout";
import { LoadingScreen } from "./components/loading-screen";
import { UpdateNotificationToast } from "./components/update-notification";

export function App() {
  const uiState = useUiState();
  // All initialization states, including failure, remain on the loading screen.
  return (
    <>
      {uiState.loading_state === "Ready" ? (
        <MainLayout uiState={uiState} />
      ) : uiState.loading_state === "Unauthenticated" ? (
        <AuthScreen />
      ) : (
        <LoadingScreen loadingState={uiState.loading_state} />
      )}
      <UpdateNotificationToast notification={uiState.update_notification} />
    </>
  );
}
