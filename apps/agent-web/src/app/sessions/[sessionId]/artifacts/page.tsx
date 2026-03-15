import { SessionArtifactsView } from "@/components/session-artifacts-view";

export default async function SessionArtifactsPage({
  params,
}: {
  params: Promise<{ sessionId: string }>;
}) {
  const { sessionId } = await params;

  return <SessionArtifactsView sessionId={sessionId} />;
}
