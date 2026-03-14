"use client";

import { useParams } from "next/navigation";

import { ChatShell } from "@/components/chat-shell";

export default function SessionChatPage() {
  const params = useParams<{ sessionId: string }>();

  return <ChatShell initialSessionId={params.sessionId} />;
}
