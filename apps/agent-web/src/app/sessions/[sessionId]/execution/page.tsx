import { SessionExecutionView } from "@/components/session-execution-view";

export default async function SessionExecutionPage({
  params,
}: {
  params: Promise<{ sessionId: string }>;
}) {
  const { sessionId } = await params;

  return <SessionExecutionView sessionId={sessionId} />;
}
