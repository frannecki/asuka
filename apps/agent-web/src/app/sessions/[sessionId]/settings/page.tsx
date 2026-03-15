import { SessionSettingsView } from "@/components/session-settings-view";

export default async function SessionSettingsPage({
  params,
}: {
  params: Promise<{ sessionId: string }>;
}) {
  const { sessionId } = await params;

  return <SessionSettingsView sessionId={sessionId} />;
}
