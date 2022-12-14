/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.19.0.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

import { CosmWasmClient, SigningCosmWasmClient, ExecuteResult } from "@cosmjs/cosmwasm-stargate";
import { Coin, StdFee } from "@cosmjs/amino";
import { ExecuteMsg, Uint128, Addr, InstantiateMsg, QueryMsg } from "./FastOracle.types";
export interface FastOracleReadOnlyInterface {
  contractAddress: string;
  price: () => Promise<PriceResponse>;
}
export class FastOracleQueryClient implements FastOracleReadOnlyInterface {
  client: CosmWasmClient;
  contractAddress: string;

  constructor(client: CosmWasmClient, contractAddress: string) {
    this.client = client;
    this.contractAddress = contractAddress;
    this.price = this.price.bind(this);
  }

  price = async (): Promise<PriceResponse> => {
    return this.client.queryContractSmart(this.contractAddress, {
      price: {}
    });
  };
}
export interface FastOracleInterface extends FastOracleReadOnlyInterface {
  contractAddress: string;
  sender: string;
  update: ({
    price
  }: {
    price: Uint128;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
  owner: ({
    owner
  }: {
    owner: Addr;
  }, fee?: number | StdFee | "auto", memo?: string, funds?: Coin[]) => Promise<ExecuteResult>;
}
export class FastOracleClient extends FastOracleQueryClient implements FastOracleInterface {
  client: SigningCosmWasmClient;
  sender: string;
  contractAddress: string;

  constructor(client: SigningCosmWasmClient, sender: string, contractAddress: string) {
    super(client, contractAddress);
    this.client = client;
    this.sender = sender;
    this.contractAddress = contractAddress;
    this.update = this.update.bind(this);
    this.owner = this.owner.bind(this);
  }

  update = async ({
    price
  }: {
    price: Uint128;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      update: {
        price
      }
    }, fee, memo, funds);
  };
  owner = async ({
    owner
  }: {
    owner: Addr;
  }, fee: number | StdFee | "auto" = "auto", memo?: string, funds?: Coin[]): Promise<ExecuteResult> => {
    return await this.client.execute(this.sender, this.contractAddress, {
      owner: {
        owner
      }
    }, fee, memo, funds);
  };
}