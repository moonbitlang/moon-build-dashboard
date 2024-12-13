import { useEffect, useState, Fragment } from "react";

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

type Platform = "mac" | "windows" | "linux";

interface PlatformData {
  mac: MoonBuildDashboard | null;
  windows: MoonBuildDashboard | null;
  linux: MoonBuildDashboard | null;
}

async function get_data(platform: Platform): Promise<MoonBuildDashboard> {
  const url = `${platform}/latest_data.jsonl.gz`;
  const response = await fetch(url, {
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

  // const url = `${platform}/latest_data.json`;
  // const response = await fetch(url);
  // if (!response.ok) {
  //   throw new Error(`HTTP error! status: ${response.status}`);
  // }
  // const text = await response.text();
  // return JSON.parse(text);
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
  const [platformData, setPlatformData] = useState<PlatformData>({
    mac: null,
    windows: null,
    linux: null
  });
  const [error, setError] = useState<string | null>(null);
  const [selectedData, setSelectedData] = useState<ExecuteResult | null>(null);
  const [modalTitle, setModalTitle] = useState("");
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [expandedItems, setExpandedItems] = useState<Set<number>>(new Set());

  const fetchData = async () => {
    try {
      const [macData, windowsData, linuxData] = await Promise.all([
        get_data("mac"),
        get_data("windows"),
        get_data("linux")
      ]);
      setPlatformData({
        mac: macData,
        windows: windowsData,
        linux: linuxData
      });
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

  const handleExpandToggle = (index: number) => {
    const newExpandedItems = new Set(expandedItems);
    if (expandedItems.has(index)) {
      newExpandedItems.delete(index);
    } else {
      newExpandedItems.add(index);
    }
    setExpandedItems(newExpandedItems);
  };

  const handleExpandAll = () => {
    if (!platformData.mac) return;
    const allIndices = new Set(platformData.mac.stable_release_data.map((_, index) => index));
    setExpandedItems(expandedItems.size === allIndices.size ? new Set() : allIndices);
  };

  const getStatusStyle = (status: Status): string => {
    return status === "Success"
      ? "bg-green-200 text-green-800"
      : "bg-red-200 text-red-800";
  };
  
  const getStatusText = (status: Status, elapsed: number | null): string => {
    return status === "Success" ? `${elapsed ?? '-'}` : "x";
  };

  const renderAllPlatformsData = () => {
    if (!platformData.mac || !platformData.windows || !platformData.linux) {
      return null;
    }

    return platformData.mac.stable_release_data.map((_, index) => {
      const macData = platformData.mac!;
      const stableEntry = macData.stable_release_data[index];
      if (!stableEntry) return null;

      const source = macData.sources[stableEntry.source];
      const isGit = "Git" in source;
      const versions = isGit ? source.Git.rev : source.MooncakesIO.version;
      const summary = generateSummary(index, platformData);

      // 渲染主行（包含摘要）
      const mainRow = (
        <tr key={`summary-${index}`} className="border-b hover:bg-gray-50 text-sm">
          <td className="py-2 px-4">
            <div className="flex items-center">
              <button
                onClick={() => handleExpandToggle(index)}
                className="mr-2 text-gray-500 hover:text-gray-700 focus:outline-none"
              >
                {expandedItems.has(index) ? "▼" : "▶"}
              </button>
              {isGit ? (
                <Fragment>
                  <i className="fab fa-github text-gray-700 mr-2"></i>
                  <a
                    href={source.Git.url}
                    className="text-blue-600 hover:text-blue-800"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    {source.Git.url.replace("https://github.com/", "")}
                  </a>
                </Fragment>
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
            </div>
          </td>
          <td className="py-2 px-4 text-gray-500">
            {isGit ? (
              <a
                href={`${source.Git.url}/tree/${versions[0]}`}
                className="text-blue-600 hover:text-blue-800"
                target="_blank"
                rel="noopener noreferrer"
              >
                {versions[0]}
              </a>
            ) : (
              versions[0]
            )}
          </td>
          <td 
            colSpan={18}
            className={`py-2 px-4 ${
              summary.status === 'success' ? 'text-green-600 bg-green-50' :
              summary.status === 'warning' ? 'text-yellow-600 bg-yellow-50' :
              'text-red-600 bg-red-50'
            }`}
          >
            {summary.text}
          </td>
        </tr>
      );

      // 详细数据行添加 hover 效果
      const detailRows = expandedItems.has(index) ? (
        ["mac", "windows", "linux"].map(platform => {
          const data = platformData[platform as Platform];
          if (!data) return null;

          const stableEntry = data.stable_release_data[index];
          const bleedingEntry = data.bleeding_release_data[index];
          if (!stableEntry) return null;

          return (
            <tr 
              key={`${platform}-${index}`} 
              className={`border-b text-sm transform transition-transform hover:scale-[1.01] hover:shadow-md ${
                platform === "windows" ? "bg-gray-50" : 
                platform === "linux" ? "bg-blue-50/20" : ""
              }`}
            >
              <td className="py-2 px-4 pl-10 text-gray-500">
                {platform.charAt(0).toUpperCase() + platform.slice(1)}
              </td>
              <td className="py-2 px-4 text-gray-500">
                {/* 空单元格，保持布局一致 */}
              </td>
              
              {stableEntry.cbts[0] ? (
                <>
                  {['check', 'build', 'test'].map(phase => (
                    ['wasm', 'wasm_gc', 'js'].map(backend => {
                      const result = stableEntry.cbts[0]?.[phase as keyof CBT]?.[backend as keyof BackendState];
                      if (!result) return null;
                      
                      return (
                        <td
                          key={`${phase}-${backend}`}
                          className={`py-2 px-4 border-r border-b text-center cursor-pointer hover:opacity-80 ${
                            getStatusStyle(result.status)
                          }`}
                          onClick={() => handleResultClick(result, `stable - ${phase} - ${backend}`)}
                        >
                          {getStatusText(result.status, result.elapsed)}
                        </td>
                      );
                    })
                  ))}
                </>
              ) : (
                <td colSpan={9} className="py-2 px-4 text-center text-gray-500">
                  No stable data available
                </td>
              )}
              
              {bleedingEntry?.cbts[0] ? (
                <>
                  {['check', 'build', 'test'].map(phase => (
                    ['wasm', 'wasm_gc', 'js'].map(backend => {
                      const result = bleedingEntry.cbts[0]?.[phase as keyof CBT]?.[backend as keyof BackendState];
                      if (!result) return null;
                      
                      return (
                        <td
                          key={`bleeding-${phase}-${backend}`}
                          className={`py-2 px-4 border-r border-b text-center cursor-pointer hover:opacity-80 ${
                            getStatusStyle(result.status)
                          }`}
                          onClick={() => handleResultClick(result, `bleeding - ${phase} - ${backend}`)}
                        >
                          {getStatusText(result.status, result.elapsed)}
                        </td>
                      );
                    })
                  ))}
                </>
              ) : (
                <td colSpan={9} className="py-2 px-4 text-center text-gray-500">
                  No bleeding data available
                </td>
              )}
            </tr>
          );
        })
      ) : null;

      return [mainRow, detailRows];
    }).flat(2).filter(Boolean);
  };

  // 检查某个 CBT 是否全部成功
  const isAllSuccess = (index: number, data: PlatformData, useBleedingEdge: boolean = false) => {
    const platforms: Platform[] = ["mac", "windows", "linux"];
    return platforms.every(platform => {
      const platformData = data[platform];
      if (!platformData) return false;
      
      const entry = useBleedingEdge ? 
        platformData.bleeding_release_data[index] : 
        platformData.stable_release_data[index];
      if (!entry) return false;
      
      return entry.cbts.every(cbt => {
        if (!cbt) return false;
        return ['check', 'build', 'test'].every(phase => 
          ['wasm', 'wasm_gc', 'js'].every(backend => 
            cbt[phase as keyof CBT][backend as keyof BackendState].status === "Success"
          )
        );
      });
    });
  };

  // 检查单个项目的状态
  const checkItemStatus = (cbt: CBT | null, phase: string, backend: string) => {
    if (!cbt) return false;
    return cbt[phase as keyof CBT][backend as keyof BackendState].status === "Success";
  };

  // 检查工具链版本之间的差异（重点一）
  const checkToolchainDifference = (index: number, data: PlatformData) => {
    const platforms: Platform[] = ["mac", "windows", "linux"];
    const phases = ['check', 'build', 'test'];
    const backends = ['wasm', 'wasm_gc', 'js'];

    for (const platform of platforms) {
      const platformData = data[platform];
      if (!platformData) continue;

      const stableEntry = platformData.stable_release_data[index];
      const bleedingEntry = platformData.bleeding_release_data[index];
      if (!stableEntry || !bleedingEntry) continue;

      for (const phase of phases) {
        for (const backend of backends) {
          const stableSuccess = stableEntry.cbts.some(cbt => checkItemStatus(cbt, phase, backend));
          const bleedingSuccess = bleedingEntry.cbts.some(cbt => checkItemStatus(cbt, phase, backend));
          
          if (stableSuccess && !bleedingSuccess) {
            return `Regression detected: ${backend} ${phase} passed in stable but failed in bleeding`;
          }
        }
      }
    }
    return null;
  };

  // 检查操作系统之间的差异（重点二）
  const checkPlatformDifference = (index: number, data: PlatformData, useBleedingEdge: boolean = false) => {
    const platforms: Platform[] = ["mac", "windows", "linux"];
    const phases = ['check', 'build', 'test'];
    const backends = ['wasm', 'wasm_gc', 'js'];

    for (const phase of phases) {
      for (const backend of backends) {
        const results = new Map<Platform, boolean>();
        
        for (const platform of platforms) {
          const platformData = data[platform];
          if (!platformData) continue;
          
          const entry = useBleedingEdge ? 
            platformData.bleeding_release_data[index] : 
            platformData.stable_release_data[index];
          if (!entry) continue;
          
          results.set(platform, entry.cbts.some(cbt => checkItemStatus(cbt, phase, backend)));
        }

        const successPlatforms = Array.from(results.entries()).filter(([_, success]) => success).map(([platform]) => platform);
        const failurePlatforms = Array.from(results.entries()).filter(([_, success]) => !success).map(([platform]) => platform);

        if (successPlatforms.length > 0 && failurePlatforms.length > 0) {
          return `Platform inconsistency: ${backend} ${phase} passed on ${successPlatforms.join(', ')} but failed on ${failurePlatforms.join(', ')}`;
        }
      }
    }
    return null;
  };

  // 检查后端之间的差异（重点三）
  const checkBackendDifference = (index: number, data: PlatformData, useBleedingEdge: boolean = false) => {
    const platforms: Platform[] = ["mac", "windows", "linux"];
    const phases = ['check', 'build', 'test'];
    const backends = ['wasm', 'wasm_gc', 'js'];

    for (const platform of platforms) {
      const platformData = data[platform];
      if (!platformData) continue;

      const entry = useBleedingEdge ? 
        platformData.bleeding_release_data[index] : 
        platformData.stable_release_data[index];
      if (!entry) continue;

      for (const phase of phases) {
        const backendResults = backends.map(backend => ({
          backend,
          success: entry.cbts.some(cbt => checkItemStatus(cbt, phase, backend))
        }));

        const successBackends = backendResults.filter(r => r.success).map(r => r.backend);
        const failureBackends = backendResults.filter(r => !r.success).map(r => r.backend);

        if (successBackends.length > 0 && failureBackends.length > 0) {
          return `Backend inconsistency: ${phase} passed on ${successBackends.join(', ')} but failed on ${failureBackends.join(', ')}`;
        }
      }
    }
    return null;
  };

  // 检查构建阶段之间的差异（重点四）
  const checkPhasesDifference = (index: number, data: PlatformData, useBleedingEdge: boolean = false) => {
    const platforms: Platform[] = ["mac", "windows", "linux"];
    const phases = ['check', 'build', 'test'];
    const backends = ['wasm', 'wasm_gc', 'js'];

    for (const platform of platforms) {
      const platformData = data[platform];
      if (!platformData) continue;

      const entry = useBleedingEdge ? 
        platformData.bleeding_release_data[index] : 
        platformData.stable_release_data[index];
      if (!entry) continue;

      for (const backend of backends) {
        const phaseResults = phases.map(phase => ({
          phase,
          success: entry.cbts.some(cbt => checkItemStatus(cbt, phase, backend))
        }));

        const successPhases = phaseResults.filter(r => r.success).map(r => r.phase);
        const failurePhases = phaseResults.filter(r => !r.success).map(r => r.phase);

        if (successPhases.length > 0 && failurePhases.length > 0) {
          return `Phase inconsistency: passed ${successPhases.join(', ')} but failed ${failurePhases.join(', ')}`;
        }
      }
    }
    return null;
  };

  // 生成项目状态摘要
  const generateSummary = (index: number, data: PlatformData): { text: string; status: 'success' | 'warning' | 'error' } => {
    // 首先检查是否所有测试都通过
    const stableSuccess = isAllSuccess(index, data);
    const bleedingSuccess = isAllSuccess(index, data, true);

    if (stableSuccess && bleedingSuccess) {
      return { text: "All passed", status: 'success' };
    }

    // 按优先级检查各种差异
    const toolchainDiff = checkToolchainDifference(index, data);
    if (toolchainDiff) {
      return { text: toolchainDiff, status: 'error' };
    }

    const platformDiffStable = checkPlatformDifference(index, data);
    const platformDiffBleeding = checkPlatformDifference(index, data, true);
    if (platformDiffStable || platformDiffBleeding) {
      return { text: platformDiffStable || platformDiffBleeding!, status: 'error' };
    }

    const backendDiffStable = checkBackendDifference(index, data);
    const backendDiffBleeding = checkBackendDifference(index, data, true);
    if (backendDiffStable || backendDiffBleeding) {
      return { text: backendDiffStable || backendDiffBleeding!, status: 'error' };
    }

    const phaseDiffStable = checkPhasesDifference(index, data);
    const phaseDiffBleeding = checkPhasesDifference(index, data, true);
    if (phaseDiffStable || phaseDiffBleeding) {
      return { text: phaseDiffStable || phaseDiffBleeding!, status: 'error' };
    }

    // 如果没有发现具体的差异模式，返回一般性的失败信息
    if (!stableSuccess || !bleedingSuccess) {
      return { text: `Other failure`, status: 'error' };
    }

    return { text: "No data available", status: 'error' };
  };

  return (
    <div className="flex flex-col h-screen bg-gray-100">
      <div className="flex-none px-8 py-4">
        <div className="flex justify-between items-center">
          <h1 className="text-2xl font-bold">Moon Build Dashboard</h1>
          <button onClick={handleExpandAll} className="px-4 py-2 text-sm bg-white border rounded-lg shadow-sm hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-blue-500">
            {expandedItems.size > 0 ? "Collapse All" : "Expand All"}
          </button>
        </div>
      </div>

      {error ? (
        <div className="flex-none px-8 py-4">
          <p className="text-red-500 text-center">{error}</p>
        </div>
      ) : platformData.mac ? (
        <div className="flex-1 flex flex-col min-h-0 px-4 overflow-auto">
          <div className="relative rounded-lg bg-white shadow-lg">
            <div className="sticky top-0 z-10 bg-white">
              <table className="w-full table-fixed">
                <colgroup>
                  <col className="w-[15%]" />
                  <col className="w-[15%]" />
                  {Array(18).fill(null).map((_, i) => (
                    <col key={i} className="w-[3.888888%]" />
                  ))}
                </colgroup>
                <thead>
                  <tr>
                    <th rowSpan={3} className="py-2 px-4 text-left border-r bg-gray-200">Repository</th>
                    <th rowSpan={3} className="py-2 px-4 text-left border-r bg-gray-200">Version</th>
                    <th colSpan={9} className="py-2 px-4 text-center bg-green-500 text-white border-r">
                      Stable Release
                      <div className="text-xs mt-1 font-normal">
                        {platformData.mac.stable_toolchain_version.moon_version} / moonc {platformData.mac.stable_toolchain_version.moonc_version}
                      </div>
                    </th>
                    <th colSpan={9} className="py-2 px-4 text-center bg-red-600 text-white relative overflow-hidden">
                      <span className="absolute inset-0 flex items-center justify-left text-6xl text-yellow-900 opacity-40">⚡️</span>
                      <div className="relative">
                        Bleeding Edge Release
                        <div className="text-xs mt-1 font-normal">
                          {platformData.mac.bleeding_toolchain_version.moon_version} / moonc {platformData.mac.bleeding_toolchain_version.moonc_version}
                        </div>
                      </div>
                    </th>
                  </tr>
                  <tr>
                    {['Check(ms)', 'Build(ms)', 'Test(ms)', 'Check(ms)', 'Build(ms)', 'Test(ms)'].map((text, i) => (
                      <th 
                        key={i} 
                        colSpan={3} 
                        className={`py-1 px-4 text-center text-sm bg-gray-100 border-b border-gray-200 ${i < 5 ? 'border-r' : ''}`}
                      >
                        {text}
                      </th>
                    ))}
                  </tr>
                  <tr>
                    {Array(18).fill(null).map((_, i) => (
                      <th 
                        key={i}
                        className={`py-1 px-4 text-center text-xs bg-gray-100 border-b border-gray-200 ${i < 17 ? 'border-r' : ''}`}
                      >
                        {['wasm', 'wasm gc', 'js'][i % 3]}
                      </th>
                    ))}
                  </tr>
                </thead>
              </table>
            </div>
            
            <div>
              <table className="w-full table-fixed">
                <colgroup>
                  <col className="w-[15%]" />
                  <col className="w-[15%]" />
                  {Array(18).fill(null).map((_, i) => (
                    <col key={i} className="w-[3.888888%]" />
                  ))}
                </colgroup>
                <tbody className="bg-white divide-y divide-gray-200">
                  {renderAllPlatformsData()}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      ) : (
        <div className="flex-none px-8 py-4">
          <p>Loading...</p>
        </div>
      )}

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
