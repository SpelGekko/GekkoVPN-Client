"use client";

import React, { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { invoke } from '@tauri-apps/api/core';
import { measureLatency } from "@/app/utils/serverLatency"; 

interface Server {
  id: number;
  name: string;
  ip: string;
  location: string; 
  latency?: number; 
}

interface User {
  username: string;
  email: string;
  plan: string;
  memberSince: string; 
}

const Dashboard = () => {
  const [isConnected, setIsConnected] = useState(false);
  const [selectedServer, setSelectedServer] = useState<Server | null>(null);
  const [servers, setServers] = useState<Server[]>([]);
  const [filteredServers, setFilteredServers] = useState<Server[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [locationFilter, setLocationFilter] = useState("");
  const [countryFilter, setCountryFilter] = useState("");
  const [currentPage, setCurrentPage] = useState(0);
  const [message, setMessage] = useState("");
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const router = useRouter();

  const serversPerPage = 5;

  const fetchUserInfo = async () => {
    try {
      const token = localStorage.getItem("vpn-token");
      if (!token) {
        console.warn("No token found. Redirecting to login.");
        router.push("/");
        return;
      }

      const apiBaseUrl = process.env.NEXT_PUBLIC_BACKEND_API_BASE_URL;
      const apiKeyClient = process.env.NEXT_PUBLIC_API_KEY_CLIENT;

      if (!apiKeyClient) {
        throw new Error("API key is missing in the environment variables.");
      }

      console.log("Fetching user information...");
      const response = await fetch(`${apiBaseUrl}/api/users/account`, {
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token}`,
          "X-API-KEY": apiKeyClient,
        },
      });

      if (!response.ok) {
        console.warn("Failed to fetch user information. Logging out.");
        localStorage.removeItem("vpn-token");
        router.push("/");
        return;
      }

      const data = await response.json();
      console.log("User details fetched from API:", data);

      if (data.profile) {
        setUser({
          username: data.profile.username,
          email: data.profile.email,
          plan: data.subscription?.plan || "Free",
          memberSince: data.profile.memberSince,
        });
      } else {
        console.warn("No profile object found in API response.");
      }
    } catch (error) {
      console.error("Error fetching user information:", error);
      setMessage("Failed to load user information. Please log in again.");
      localStorage.removeItem("vpn-token");
      router.push("/");
    }
  };

const fetchServers = async () => {
    try {
        const apiBaseUrl = process.env.NEXT_PUBLIC_BACKEND_API_BASE_URL;
        console.log("Fetching servers...");
        const response = await fetch(`${apiBaseUrl}/api/servers`, {
            headers: {
                "Content-Type": "application/json",
            },
        });

        if (!response.ok) {
            throw new Error("Failed to fetch servers");
        }

        const data = await response.json();
        console.log("Servers fetched from API:", data);

        const serversWithLatency = await Promise.all(
            data.servers.map(async (server: Server) => {
                const latency = await measureLatency(server.ip);
                return { ...server, latency };
            })
        );

        setServers(serversWithLatency);
        setFilteredServers(serversWithLatency);
    } catch (error) {
        console.error("Error fetching servers:", error);
        setMessage("Failed to load servers. Please try again later.");
    }
};

  useEffect(() => {
    const associateCredentials = async () => {
      if (user?.username) {
        try {
          await invoke('associate_username', { 
            username: user.username 
          });
        } catch (error) {
        }
      }
    };
  
    associateCredentials();
  }, [user]); // Run when user data is loaded

  const handleConnect = async (server: Server) => {
    if (!user) {
      setMessage("User information not available. Please try logging in again.");
      return;
    }
  
    setMessage("");
    setIsLoading(true);
    try {
      if (isConnected && selectedServer?.id === server.id) {
        const response = await invoke('disconnect_vpn');
        console.log(`Disconnected from server: ${server.name}`);
        setIsConnected(false);
        setSelectedServer(null);
        setMessage(response as string);
      } else {
        // Get the stored password for the user
        const password = await invoke('get_vpn_password', { 
          username: user.username 
        });
  
        if (!password) {
          setMessage("VPN credentials not found. Please log in again.");
          router.push("/");
          return;
        }
  
        const serverDirName = server.name.replace(/\s+/g, '-');
        const response = await invoke('connect_vpn', {
          serverName: serverDirName,
          username: user.username
        });
        
        console.log(`Connected to server: ${server.name}`);
        setIsConnected(true);
        setSelectedServer(server);
        setMessage(response as string);
      }
    } catch (error) {
      console.error('VPN connection error:', error);
      setMessage(`Failed to ${isConnected ? 'disconnect from' : 'connect to'} VPN: ${error}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleLogout = async () => {
    try {
      // Disconnect VPN if connected
      if (isConnected) {
        await invoke('disconnect_vpn');
      }
      
      // Clear VPN credentials if user exists
      if (user?.username) {
        await invoke('clear_credentials', { 
          username: user.username 
        });
      }
      
      // Clear local storage
      localStorage.removeItem("vpn-token");
      localStorage.removeItem("vpn-username");
      
      // Redirect to login page
      router.push("/");
    } catch (error) {
      console.error('Error during logout:', error);
      setMessage("Failed to logout properly. Please try again.");
    }
  };

  const getCountryCode = (location: string) => {
    const parts = location.split(", ");
    return parts.length > 1 ? parts[1] : "";
  };

  useEffect(() => {
    const filtered = servers.filter((server) => {
      const matchesSearch = server.name
        .toLowerCase()
        .includes(searchQuery.toLowerCase());
      const matchesLocation =
        locationFilter === "" || server.location === locationFilter;
      const matchesCountry =
        countryFilter === "" ||
        getCountryCode(server.location)?.toUpperCase() === countryFilter;

      return matchesSearch && matchesLocation && matchesCountry;
    });

    setFilteredServers(filtered);
    setCurrentPage(0);
  }, [searchQuery, locationFilter, countryFilter, servers]);

  useEffect(() => {
    fetchUserInfo();
  }, [router]);

  useEffect(() => {
    fetchServers();
  }, []);

  useEffect(() => {
    const checkVpnStatus = async () => {
      try {
        const status = await invoke('get_vpn_status');
        setIsConnected(status as boolean);
      } catch (error) {
        console.error('Error checking VPN status:', error);
      }
    };

    checkVpnStatus();
  }, []);

  const paginatedServers = filteredServers.slice(
    currentPage * serversPerPage,
    (currentPage + 1) * serversPerPage
  );

  return (
    <div className="min-h-screen bg-gradient-to-b from-white via-emerald-50 to-white dark:from-gray-900 dark:via-gray-800 dark:to-gray-900">
      <div className="max-w-4xl mx-auto py-10 px-6">
        <h1 className="text-3xl font-bold text-center text-gray-800 dark:text-white">
          GekkoVPN Dashboard
        </h1>

        {user ? (
          <div className="mt-8 p-6 bg-white rounded-lg shadow-lg dark:bg-gray-800">
            <div className="flex justify-between items-center">
              <h2 className="text-2xl font-bold text-gray-800 dark:text-white">
                Welcome, {user.username}!
              </h2>
              <button
                onClick={handleLogout}
                className="px-4 py-2 text-white bg-red-500 rounded-lg hover:bg-red-600 focus:outline-none focus:ring-2 focus:ring-red-500"
              >
                Logout
              </button>
            </div>
            <div className="mt-4 grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <p className="text-sm font-medium text-gray-800 dark:text-gray-200">
                  Email:
                </p>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  {user.email}
                </p>
              </div>
              <div>
                <p className="text-sm font-medium text-gray-800 dark:text-gray-200">
                  Plan:
                </p>
                <p className="text-sm font-bold text-gray-800 dark:text-gray-200">
                  {user.plan}
                </p>
              </div>
              <div className="sm:col-span-2">
                <p className="text-sm font-medium text-gray-800 dark:text-gray-200">
                  Member Since:
                </p>
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  {new Date(user.memberSince).toLocaleDateString()}
                </p>
              </div>
            </div>
          </div>
        ) : (
          <p className="mt-8 text-center text-gray-500 dark:text-gray-400">
            Loading user information...
          </p>
        )}

        <div className="mt-8 p-6 bg-white rounded-lg shadow-lg dark:bg-gray-800">
          <h2 className="text-xl font-semibold text-gray-800 dark:text-white">
            Available Servers
          </h2>

          <div className="mt-4 flex flex-col sm:flex-row sm:items-center sm:justify-between space-y-4 sm:space-y-0">
            <input
              type="text"
              placeholder="Search servers..."
              className="w-full sm:w-1/3 px-4 py-2 border rounded-lg text-gray-800 dark:text-gray-200 bg-gray-50 dark:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-emerald-500"
              onChange={(e) => setSearchQuery(e.target.value)}
            />

            <select
              className="w-full sm:w-1/4 px-4 py-2 border rounded-lg text-gray-800 dark:text-gray-200 bg-gray-50 dark:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-emerald-500"
              onChange={(e) => setLocationFilter(e.target.value)}
            >
              <option value="">All Locations</option>
              {Array.from(new Set(servers.map((server) => server.location))).map(
                (location) => (
                  <option key={location} value={location}>
                    {location}
                  </option>
                )
              )}
            </select>

            <select
              className="w-full sm:w-1/4 px-4 py-2 border rounded-lg text-gray-800 dark:text-gray-200 bg-gray-50 dark:bg-gray-700 focus:outline-none focus:ring-2 focus:ring-emerald-500"
              onChange={(e) => setCountryFilter(e.target.value)}
            >
              <option value="">All Countries</option>
              {Array.from(
                new Set(
                  servers.map((server) =>
                    getCountryCode(server.location)?.toUpperCase()
                  )
                )
              )
                .filter((country) => country)
                .map((country) => (
                  <option key={country} value={country}>
                    {country}
                  </option>
                ))}
            </select>
          </div>

          <ul className="mt-4 space-y-4">
            {paginatedServers.map((server) => {
              const countryCode = getCountryCode(server.location);
              const flagUrl = `https://flagcdn.com/w40/${countryCode?.toLowerCase()}.png`;

              return (
                <li
                  key={server.id}
                  className={`flex items-center justify-between p-4 border rounded-lg ${
                    selectedServer?.id === server.id
                      ? "bg-emerald-100 dark:bg-emerald-900"
                      : "bg-gray-50 dark:bg-gray-700"
                  }`}
                >
                  <div className="flex items-center space-x-4">
                    <img
                      src={flagUrl}
                      alt={`${countryCode} flag`}
                      className="w-10 h-6 rounded shadow"
                    />
                    <div>
                      <p className="text-lg font-medium text-gray-800 dark:text-white">
                        {server.name}
                      </p>
                      <p className="text-sm text-gray-500 dark:text-gray-400">
                        {server.ip} - {server.location}
                      </p>
                      <p className="text-sm text-gray-500 dark:text-gray-400">
                        Latency: {server.latency !== undefined ? `${server.latency} ms` : "N/A"}
                      </p>
                    </div>
                  </div>
                  <button
                    onClick={() => handleConnect(server)}
                    disabled={isLoading}
                    className={`px-4 py-2 text-white rounded-lg ${
                      isLoading 
                        ? "bg-gray-400 cursor-not-allowed"
                        : isConnected && selectedServer?.id === server.id
                        ? "bg-red-500 hover:bg-red-600"
                        : "bg-emerald-500 hover:bg-emerald-600"
                    } focus:outline-none focus:ring-2 focus:ring-emerald-500`}
                  >
                    {isLoading 
                      ? "Loading..."
                      : isConnected && selectedServer?.id === server.id
                      ? "Disconnect"
                      : "Connect"}
                  </button>
                </li>
              );
            })}
          </ul>

          <div className="flex justify-between items-center mt-4">
            <button
              onClick={() => setCurrentPage((prev) => Math.max(prev - 1, 0))}
              disabled={currentPage === 0}
              className={`px-4 py-2 rounded-lg ${
                currentPage === 0
                  ? "bg-gray-300 text-gray-500 cursor-not-allowed"
                  : "bg-emerald-500 text-white hover:bg-emerald-600"
              }`}
            >
              Previous
            </button>
            <button
              onClick={() =>
                setCurrentPage((prev) =>
                  Math.min(
                    prev + 1,
                    Math.ceil(filteredServers.length / serversPerPage) - 1
                  )
                )
              }
              disabled={(currentPage + 1) * serversPerPage >= filteredServers.length}
              className={`px-4 py-2 rounded-lg ${
                (currentPage + 1) * serversPerPage >= filteredServers.length
                  ? "bg-gray-300 text-gray-500 cursor-not-allowed"
                  : "bg-emerald-500 text-white hover:bg-emerald-600"
              }`}
            >
              Next
            </button>
          </div>
        </div>

        {message && (
          <div className="mt-8 p-4 text-center text-sm text-gray-800 bg-emerald-100 rounded-lg dark:bg-gray-700 dark:text-gray-300">
            {message}
          </div>
        )}
      </div>
    </div>
  );
};

export default Dashboard;