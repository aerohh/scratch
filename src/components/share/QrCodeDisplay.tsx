import { createPortal } from "react-dom";
import { QRCodeSVG } from "qrcode.react";
import { Button } from "../ui";
import { DownloadIcon, ShareIcon, XIcon } from "../icons";

interface QrCodeDisplayProps {
  inviteCode: string;
  folderName: string;
  onClose: () => void;
}

export default function QrCodeDisplay({ inviteCode, folderName, onClose }: QrCodeDisplayProps) {
  const handleDownload = () => {
    const svg = document.querySelector<SVGSVGElement>("#qr-code-svg");
    if (!svg) return;

    const serializer = new XMLSerializer();
    const source = serializer.serializeToString(svg);
    const url = "data:image/svg+xml;charset=utf-8," + encodeURIComponent(source);

    const link = document.createElement("a");
    link.href = url;
    link.download = `share-${folderName.replace(/\s+/g, "-")}-qr.svg`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
  };

  const content = (
    <div
      className="fixed inset-0 z-[90] bg-black/50 backdrop-blur-sm flex items-center justify-center p-4"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="w-full max-w-sm rounded-xl border border-border bg-bg shadow-2xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-border">
          <div className="flex items-center gap-2">
            <ShareIcon className="w-4 h-4 text-accent stroke-[1.8]" />
            <h2 className="text-base font-semibold">Share QR Code</h2>
          </div>
          <button onClick={onClose} className="p-1.5 rounded-md hover:bg-bg-muted" aria-label="Close">
            <XIcon className="w-4.5 h-4.5" />
          </button>
        </div>

        <div className="p-6 flex flex-col items-center gap-4">
          <div className="bg-white p-3 rounded-lg border border-border">
            <QRCodeSVG id="qr-code-svg" value={inviteCode} size={220} level="M" includeMargin />
          </div>
          <div className="text-center">
            <div className="font-medium">{folderName}</div>
            <div className="text-xs text-text-muted">Scan this code to accept the share</div>
          </div>
        </div>

        <div className="flex justify-end px-5 py-4 border-t border-border">
          <Button variant="outline" onClick={handleDownload}>
            <DownloadIcon className="w-4 h-4 mr-2" />
            Download SVG
          </Button>
        </div>
      </div>
    </div>
  );

  return createPortal(content, document.body);
}
