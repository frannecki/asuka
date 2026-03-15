import { ChatShell } from "@/components/chat-shell";

export default async function SessionChatPage({
  params,
}: {
  params: Promise<{ sessionId: string }>;
}) {
  const { sessionId } = await params;

  return <ChatShell initialSessionId={sessionId} routeMode="session" />;
}
