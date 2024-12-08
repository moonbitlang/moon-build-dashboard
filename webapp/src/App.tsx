import { useEffect, useState } from "react";

type MooncakeSource = 
  | { MooncakesIO: { name: string; version: string[]; index: number } }
  | { Git: { url: string; rev: string[]; index: number } };

type ToolChainLabel = "Stable" | "Bleeding";

interface ToolChainVersion {
  label: ToolChainLabel;
  moon_version: string;
  moonc_version: string;
}

interface MoonBuildDashboard {
  run_id: string;
  run_number: string;
  start_time: string;
  sources: MooncakeSource[];
  stable_toolchain_version: ToolChainVersion;
  stable_release_data: BuildState[];
  bleeding_toolchain_version: ToolChainVersion;
  bleeding_release_data: BuildState[];
}

type Status = "Success" | "Failure";

interface ExecuteResult {
  status: Status;
  start_time: string;
  elapsed: number;
  stdout: string;
  stderr: string;
}

interface BackendState {
  wasm: ExecuteResult;
  wasm_gc: ExecuteResult;
  js: ExecuteResult;
}

interface CBT {
  check: BackendState;
  build: BackendState;
  test: BackendState;
}

interface BuildState {
  source: number;
  cbts: (CBT | null)[];
}

async function get_data(): Promise<MoonBuildDashboard> {
  const response = await fetch('/latest_data.jsonl.gz', {
    headers: {
      'Accept-Encoding': 'gzip'
    }
  });
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  const blob = await response.blob();
  const ds = new DecompressionStream('gzip');
  const decompressedStream = blob.stream().pipeThrough(ds);
  const decompressedBlob = await new Response(decompressedStream).blob();
  const text = await decompressedBlob.text();
  return JSON.parse(text);
}

interface ModalProps {
  isOpen: boolean;
  onClose: () => void;
  data: ExecuteResult;
  title: string;
}

const DetailModal: React.FC<ModalProps> = ({ isOpen, onClose, data, title }) => {
  if (!isOpen) return null;

  useEffect(() => {
    const handleEsc = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };
    window.addEventListener('keydown', handleEsc);

    return () => {
      window.removeEventListener('keydown', handleEsc);
    };
  }, [onClose]);


  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg p-6 max-w-4xl w-full max-h-[90vh] overflow-y-auto">
        <div className="flex justify-between items-center mb-4">
          <h3 className="text-xl font-bold">{title}</h3>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-700"
          >
            ✕
          </button>
        </div>
        <div className="space-y-4">
          <div className="flex space-x-4">
            <p className="font-semibold">Status: 
              <span className={data.status === "Success" ? "text-green-600" : "text-red-600"}>
                {data.status}
              </span>
            </p>
            <p className="font-semibold">Start Time: {data.start_time}</p>
            <p className="font-semibold">Elapsed: {data.elapsed}ms</p>
          </div>
          
          {/* Stdout */}
          <div>
            <div className="text-gray-700 font-semibold mb-2">stdout</div>
            <div className="bg-gray-900 rounded-lg p-4 font-mono text-sm">
              <div className="text-gray-300 whitespace-pre-wrap overflow-x-auto">
                {data.stdout || "no stdout output"}
              </div>
            </div>
          </div>
          
          {/* Stderr */}
          <div>
            <div className="text-gray-700 font-semibold mb-2">stderr</div>
            <div className="bg-gray-900 rounded-lg p-4 font-mono text-sm">
              <div className="text-gray-300 whitespace-pre-wrap overflow-x-auto">
                {data.stderr || "no stderr output"}
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

const App = () => {
  const [data, setData] = useState<MoonBuildDashboard | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedData, setSelectedData] = useState<ExecuteResult | null>(null);
  const [modalTitle, setModalTitle] = useState("");
  const [isModalOpen, setIsModalOpen] = useState(false);

  const fetchData = async () => {
    try {
      const parsedData = await get_data();
      setData(parsedData);
    } catch (err) {
      if (err instanceof Error) {
        setError(err.message);
      } else {
        setError("Unknown error occurred");
      }
    }
  };

  useEffect(() => {
    fetchData();
  }, []);

  const handleResultClick = (result: ExecuteResult, title: string) => {
    setSelectedData(result);
    setModalTitle(title);
    setIsModalOpen(true);
  };

  const renderBackendState = (
    backendState: BackendState, 
    phase: string, 
    variant: 'stable' | 'bleeding',
    stableCBT?: CBT | null
  ) => {
    const highlightDifference = (
      stable: ExecuteResult,
      bleeding: ExecuteResult
    ) => stable.status !== bleeding.status ? "bg-yellow-100" : "";

    const getStatusStyle = (status: Status): string => {
      return status === "Success"
        ? "bg-green-200 text-green-800"
        : "bg-red-200 text-red-800";
    };
    
    const getStatusText = (status: Status, elapsed: number | null): string => {
      return status === "Success" ? `${elapsed ?? '-'}` : "x";
    };

    return (
      <>
        {["wasm", "wasm_gc", "js"].map((key) => {
          const result = backendState[key as keyof BackendState];
          const stableResult = stableCBT?.[phase.toLowerCase() as keyof CBT]?.[key as keyof BackendState];
          
          return (
            <td
              key={`${variant}-${phase.toLowerCase()}-${key}`}
              className={`py-2 px-4 border-r cursor-pointer hover:opacity-80 
                ${getStatusStyle(result.status)}
                ${variant === 'bleeding' && stableResult ? highlightDifference(stableResult, result) : ''}`}
              onClick={() => handleResultClick(
                result,
                `${variant} - ${phase.toLowerCase()} - ${key}`
              )}
            >
              {getStatusText(result.status, result.elapsed)}
            </td>
          );
        })}
      </>
    );
  };

  const renderTableRows = (
    stableData: BuildState[],
    bleedingData: BuildState[],
    sources: MooncakeSource[]
  ) => {
    return stableData.map((stableEntry, index) => {
      const source = sources[stableEntry.source];
      const isGit = "Git" in source;
      const versions = isGit ? source.Git.rev : source.MooncakesIO.version;
      const rowSpan = versions.length; // Number of versions determines the row span
  
      return versions.map((_, versionIndex) => {
        const bleedingEntry = bleedingData[index];
        const stableCBT = stableEntry.cbts[versionIndex];
        const bleedingCBT = bleedingEntry?.cbts[versionIndex];
  
        return (
          <tr
            key={`${index}-${versionIndex}`}
            className="border-b hover:bg-gray-50 text-sm"
          >
            {/* Only display the source name in the first row */}
            {versionIndex === 0 && (
              <td className="py-2 px-4" rowSpan={rowSpan}>
                {isGit ? (
                  <>
                    <i className="fab fa-github text-gray-700 mr-2"></i>
                    <a
                      href={source.Git.url}
                      className="text-blue-600 hover:text-blue-800"
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      {source.Git.url.replace("https://github.com/", "")}
                    </a>
                  </>
                ) : (
                  <a
                    href={`https://mooncakes.io/docs/#/${source.MooncakesIO.name}/`}
                    className="text-blue-600 hover:text-blue-800"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    {source.MooncakesIO.name}
                  </a>
                )}
              </td>
            )}
            {/* Display the version without a link if it is MooncakesIO */}
            <td className="py-2 px-4 text-gray-500">
              {isGit ? (
                <a
                  href={`${source.Git.url}/tree/${versions[versionIndex]}`}
                  className="text-blue-600 hover:text-blue-800"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  {versions[versionIndex]}
                </a>
              ) : (
                versions[versionIndex]
              )}
            </td>
  
            {/* Stable Data */}
            {stableCBT ? (
              <>
                {renderBackendState(stableCBT.check, "Check", 'stable')}
                {renderBackendState(stableCBT.build, "Build", 'stable')}
                {renderBackendState(stableCBT.test, "Test", 'stable')}
              </>
            ) : (
              <td colSpan={9} className="py-2 px-4 text-center text-gray-500">
                No stable data available
              </td>
            )}
  
            {/* Bleeding Data */}
            {bleedingCBT ? (
              <>
                {renderBackendState(bleedingCBT.check, "Check", 'bleeding', stableCBT)}
                {renderBackendState(bleedingCBT.build, "Build", 'bleeding', stableCBT)}
                {renderBackendState(bleedingCBT.test, "Test", 'bleeding', stableCBT)}
              </>
            ) : (
              <td colSpan={9} className="py-2 px-4 text-center text-gray-500">
                No bleeding data available
              </td>
            )}
          </tr>
        );
      });
    });
  };
  
  return (
    <div className="p-4 bg-gray-100 min-h-screen flex justify-center">
      <div className="w-full">
        <h1 className="text-2xl font-bold mb-4">Moon Build Dashboard</h1>
  
        {error ? (
          <p className="text-red-500 text-center">{error}</p>
        ) : data ? (
          <div className="overflow-x-auto">
            <table className="min-w-full table-auto bg-white shadow-md rounded-lg overflow-hidden">
              <thead>
                <tr className="bg-gray-200">
                  <th rowSpan={3} className="py-2 px-4 text-left w-1/4 border-r">Repository</th>
                  <th rowSpan={3} className="py-2 px-4 text-left w-1/4 border-r">Version</th>
                  <th colSpan={9} className="py-2 px-4 text-center bg-green-500 text-white border-r">
                    Stable Release
                    <div className="text-xs mt-1 font-normal">
                      {data.stable_toolchain_version.moon_version} / moonc {data.stable_toolchain_version.moonc_version}
                    </div>
                  </th>
                  <th
                    colSpan={9}
                    className="py-2 px-4 text-center bg-red-600 text-white relative overflow-hidden"
                  >
                    <span className="absolute inset-0 flex items-center justify-left text-6xl text-yellow-900 opacity-40">
                      ⚡️
                    </span>
                    Bleeding Edge Release
                    <div className="text-xs mt-1 font-normal">
                      {data.bleeding_toolchain_version.moon_version} / moonc {data.bleeding_toolchain_version.moonc_version}
                    </div>
                  </th>
                </tr>
                <tr className="bg-gray-100">
                  <th colSpan={3} className="py-1 px-4 text-center text-sm border-r">Check(ms)</th>
                  <th colSpan={3} className="py-1 px-4 text-center text-sm border-r">Build(ms)</th>
                  <th colSpan={3} className="py-1 px-4 text-center text-sm">Test(ms)</th>
                  <th colSpan={3} className="py-1 px-4 text-center text-sm border-r">Check(ms)</th>
                  <th colSpan={3} className="py-1 px-4 text-center text-sm border-r">Build(ms)</th>
                  <th colSpan={3} className="py-1 px-4 text-center text-sm">Test(ms)</th>
                </tr>
                <tr className="bg-gray-100">
                  <th className="py-1 px-4 text-center text-xs border-r">wasm</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm gc</th>
                  <th className="py-1 px-4 text-center text-xs border-r">js</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm gc</th>
                  <th className="py-1 px-4 text-center text-xs border-r">js</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm gc</th>
                  <th className="py-1 px-4 text-center text-xs border-r">js</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm gc</th>
                  <th className="py-1 px-4 text-center text-xs border-r">js</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm gc</th>
                  <th className="py-1 px-4 text-center text-xs border-r">js</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm</th>
                  <th className="py-1 px-4 text-center text-xs border-r">wasm gc</th>
                  <th className="py-1 px-4 text-center text-xs border-r">js</th>
                </tr>
              </thead>
              <tbody>
                {renderTableRows(data.stable_release_data, data.bleeding_release_data, data.sources)}
              </tbody>
            </table>
          </div>
        ) : (
          <p>Loading...</p>
        )}
      </div>
      {selectedData && (
        <DetailModal
          isOpen={isModalOpen}
          onClose={() => setIsModalOpen(false)}
          data={selectedData}
          title={modalTitle}
        />
      )}
    </div>
  );
};

export default App;
